// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Synchronous bounded read control port.

use std::sync::Arc;
use std::sync::Mutex;

use super::internal::ManagedReadState;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::OpenedFileInfo;
use crate::spi::RandomAccessReadSession;

/// Thread-safe control port for bounded reads from one opened object.
///
/// Clones share one serialized provider session and one cumulative byte
/// budget. The session is never reopened by path, so replacing a path after
/// constructing this port does not redirect reads to a different object.
///
/// # Examples
///
/// ```
/// use qubit_fs::error::FsResult;
/// use qubit_fs::metadata::OpenedFileInfo;
/// use qubit_fs::spi::{ManagedReadPort, RandomAccessReadSession};
///
/// fn read_one(info: OpenedFileInfo, session: Box<dyn RandomAccessReadSession>) -> FsResult<usize> {
///     let port = ManagedReadPort::new(info, 8192, 8192, 8192, session);
///     let mut byte = [0_u8; 1];
///     port.read_at(0, &mut byte)
/// }
/// ```
#[must_use]
pub struct ManagedReadPort {
    /// Shared serialized session and cumulative byte budget for all clones.
    state: Arc<Mutex<ManagedReadState>>,
}

impl Clone for ManagedReadPort {
    /// Creates another handle to the same serialized session and byte budget.
    ///
    /// # Parameters
    /// - `self`: Port whose shared state is cloned.
    ///
    /// # Returns
    /// A port sharing the retained provider session and cumulative limits.
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }
}

impl ManagedReadPort {
    /// Creates a bounded port retaining the already-open provider session.
    ///
    /// `len` and `info` must describe the same object retained by `session`.
    /// `max_segment` limits each provider call; `max_total` counts successful
    /// bytes across all calls, including repeated ranges and cloned ports.
    ///
    /// # Parameters
    /// - `info`: Metadata observed when opening the retained object.
    /// - `len`: Readable length of the same object retained by `session`.
    /// - `max_segment`: Maximum bytes sent to one provider call.
    /// - `max_total`: Cumulative successful-byte limit shared by clones.
    /// - `session`: Already-open provider session bound to `info` and `len`.
    ///
    /// # Returns
    /// A bounded control port for the retained provider session.
    pub fn new(
        info: OpenedFileInfo,
        len: u64,
        max_segment: usize,
        max_total: u64,
        session: Box<dyn RandomAccessReadSession>,
    ) -> Self {
        Self {
            state: Arc::new(Mutex::new(ManagedReadState {
                info,
                len,
                max_segment,
                max_total,
                consumed: 0,
                session,
            })),
        }
    }

    /// Returns the identity and open-time metadata bound to this session.
    ///
    /// # Returns
    /// The opened file information, or a filesystem error if the shared state
    /// lock is poisoned.
    pub fn info(&self) -> FsResult<OpenedFileInfo> {
        self.state
            .lock()
            .map(|state| state.info.clone())
            .map_err(|_| poisoned_read_error())
    }

    /// Returns the open-time readable length; it does not imply a snapshot.
    ///
    /// # Returns
    /// The opened object's readable length, or an error if the shared state
    /// lock is poisoned.
    pub fn len(&self) -> FsResult<u64> {
        self.state
            .lock()
            .map(|state| state.len)
            .map_err(|_| poisoned_read_error())
    }

    /// Returns whether the opened object has zero readable bytes.
    pub fn is_empty(&self) -> FsResult<bool> {
        self.len().map(|length| length == 0)
    }

    /// Reads the remaining cumulative budget and the retained object's length.
    ///
    /// # Returns
    /// The remaining byte budget and open-time length, or a provider-contract
    /// error if the shared session state is poisoned.
    fn remaining_budget_and_len(&self) -> FsResult<(u64, u64)> {
        self.state
            .lock()
            .map(|state| (state.max_total.saturating_sub(state.consumed), state.len))
            .map_err(|_| poisoned_read_error())
    }

    /// Reads a complete object within `options.max_bytes`.
    ///
    /// The method reads one sentinel byte beyond the requested content limit
    /// to distinguish an exact-boundary file from an oversized file. The
    /// returned version and change observation remain unknown unless a future
    /// provider protocol supplies facts bound to this same session; in
    /// particular, `ObserveChanges` does not promise a snapshot.
    ///
    /// # Parameters
    /// - `options`: Read limit and consistency requirements.
    ///
    /// # Returns
    /// The complete bytes plus only the version/change facts known for this
    /// opened session.
    ///
    /// # Errors
    /// Returns an error for an unrepresentable sentinel, required version
    /// guarantees that this port cannot establish, allocation failure,
    /// provider failure, or content larger than the requested maximum.
    pub fn read_all(
        &self,
        options: crate::read::ReadAllOptions,
    ) -> FsResult<crate::read::ConsistentReadOutcome<Vec<u8>>> {
        use crate::read::ReadConsistencyPolicy;

        let capacity = options.sentinel_capacity()?;
        if options.consistency == ReadConsistencyPolicy::RequireVersion {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Read,
                "opened read session cannot guarantee a stable version",
            ));
        }
        let version = self.info()?.metadata().and_then(|metadata| metadata.etag().cloned());
        let (remaining_budget, len) = self.remaining_budget_and_len()?;
        let allocation_limit = capacity
            .min(usize::try_from(remaining_budget).unwrap_or(usize::MAX))
            .min(usize::try_from(len).unwrap_or(usize::MAX));
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(allocation_limit).map_err(|error| {
            FsError::with_source(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "read buffer allocation exceeds available capacity",
                error,
            )
        })?;
        let mut scratch = [0_u8; 8192];
        loop {
            let remaining = capacity - bytes.len();
            let chunk_len = remaining.min(scratch.len());
            let count = self.read_at(bytes.len() as u64, &mut scratch[..chunk_len])?;
            if count == 0 {
                return Ok(crate::read::ConsistentReadOutcome {
                    value: bytes,
                    version,
                    changed: None,
                });
            }
            if bytes.len() + count > options.max_bytes {
                return Err(FsError::new(
                    FsErrorKind::ResourceLimitExceeded,
                    FsOperation::Read,
                    "read exceeds maximum byte count",
                ));
            }
            bytes.extend_from_slice(&scratch[..count]);
        }
    }

    /// Reads a bounded prefix without claiming that the returned bytes are
    /// the complete file.
    ///
    /// # Parameters
    /// - `max_bytes`: Maximum number of prefix bytes to read.
    ///
    /// # Returns
    /// The bytes read, their opened-file information, and the termination
    /// reason.
    ///
    /// # Errors
    /// Returns allocation, provider, or managed-budget errors.
    pub fn read_prefix(&self, max_bytes: usize) -> FsResult<crate::read::PrefixReadOutcome> {
        let info = self.info()?;
        let (remaining_budget, len) = self.remaining_budget_and_len()?;
        let allocation_limit = max_bytes
            .min(usize::try_from(remaining_budget).unwrap_or(usize::MAX))
            .min(usize::try_from(len).unwrap_or(usize::MAX));
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(allocation_limit).map_err(|error| {
            FsError::with_source(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "read buffer allocation exceeds available capacity",
                error,
            )
        })?;
        let mut scratch = [0_u8; 8192];
        let mut termination = crate::read::PrefixReadTermination::LimitReached;
        while bytes.len() < max_bytes {
            let chunk_len = (max_bytes - bytes.len()).min(scratch.len());
            let count = self.read_at(bytes.len() as u64, &mut scratch[..chunk_len])?;
            if count == 0 {
                termination = crate::read::PrefixReadTermination::StreamEnded;
                break;
            }
            bytes.extend_from_slice(&scratch[..count]);
        }
        Ok(crate::read::PrefixReadOutcome::new(
            bytes,
            info,
            crate::read::ReadOptions::default(),
            max_bytes,
            termination,
        ))
    }

    /// Reads a complete bounded object and validates it as strict UTF-8.
    ///
    /// # Parameters
    /// - `options`: Read limit and consistency requirements.
    ///
    /// # Returns
    /// The decoded UTF-8 text.
    ///
    /// # Errors
    /// Returns the underlying read error or an error when the bytes are not
    /// valid UTF-8.
    pub fn read_text(&self, options: crate::read::ReadAllOptions) -> FsResult<String> {
        let outcome = self.read_all(options)?;
        crate::read::ReadAllOptions::decode_text(outcome.value)
    }

    /// Reads one bounded segment from the already-open object.
    ///
    /// A repeated range consumes the budget again. A request that would
    /// overflow its offset is rejected before provider I/O. Requests are
    /// clipped to the open-time length and the remaining cumulative budget.
    ///
    /// # Parameters
    /// - `offset`: Starting byte offset in the opened object.
    /// - `output`: Destination buffer for this read.
    ///
    /// # Returns
    /// The number of bytes read; short reads and zero at EOF are valid.
    ///
    /// # Errors
    /// Returns typed filesystem errors for invalid offsets, exhausted byte
    /// budget, provider errors, or provider over-reporting.
    ///
    /// # Panics
    /// Panics only if the internal bound that the read count fits in `u64` is
    /// violated.
    pub fn read_at(&self, offset: u64, output: &mut [u8]) -> FsResult<usize> {
        let requested = u64::try_from(output.len()).map_err(|_| {
            FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "read length cannot be represented",
            )
        })?;
        offset.checked_add(requested).ok_or_else(|| {
            FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::Read,
                "read offset range overflows",
            )
        })?;
        if output.is_empty() {
            return Ok(0);
        }
        let mut state = self.state.lock().map_err(|_| poisoned_read_error())?;
        if offset >= state.len {
            return Ok(0);
        }
        if state.max_segment == 0 {
            return Err(FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "managed read segment limit is zero",
            )
            .with_path(state.info.path().clone()));
        }
        let remaining_budget = state.max_total.saturating_sub(state.consumed);
        if remaining_budget == 0 {
            return Err(FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "managed read byte budget exhausted",
            )
            .with_path(state.info.path().clone()));
        }
        let remaining_object = state.len - offset;
        let count = output
            .len()
            .min(state.max_segment)
            .min(usize::try_from(remaining_object).unwrap_or(usize::MAX))
            .min(usize::try_from(remaining_budget).unwrap_or(usize::MAX));
        if count == 0 {
            return Ok(0);
        }
        let n = state.session.read_at(offset, &mut output[..count]).map_err(|error| {
            error
                .with_missing_context(state.info.path(), None, "managed-read")
                .with_operation(FsOperation::Read)
        })?;
        if n > count {
            return Err(FsError::new(
                FsErrorKind::ProviderContractViolation,
                FsOperation::Read,
                "provider returned more bytes than supplied buffer",
            )
            .with_path(state.info.path().clone()));
        }
        state.consumed += u64::try_from(n).expect("read count is bounded by a usize buffer");
        Ok(n)
    }
}

/// Creates the provider-contract error used when the read-session lock is
/// poisoned.
///
/// # Returns
/// An error identifying a failed managed read operation.
fn poisoned_read_error() -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::Read,
        "managed read session lock was poisoned",
    )
}
