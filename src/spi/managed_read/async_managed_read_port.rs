// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous bounded read control port.

#[cfg(feature = "async")]
use crate::error::FsError;
#[cfg(feature = "async")]
use crate::error::FsErrorKind;
#[cfg(feature = "async")]
use crate::error::FsOperation;
#[cfg(feature = "async")]
use crate::error::FsResult;
#[cfg(feature = "async")]
use crate::metadata::OpenedFileInfo;

/// Mutable asynchronous control port for bounded reads from one opened object.
///
/// Calls borrow the port mutably for the entire operation, which serializes
/// reads without retaining a borrowed SPI future or requiring a runtime.
///
/// # Examples
///
/// ```
/// use qubit_fs::metadata::OpenedFileInfo;
/// use qubit_fs::spi::{AsyncManagedReadPort, AsyncRandomAccessReadSession};
/// use std::pin::Pin;
///
/// async fn read_one(
///     info: OpenedFileInfo,
///     session: Pin<Box<dyn AsyncRandomAccessReadSession>>,
/// ) {
///     let mut port = AsyncManagedReadPort::new(info, 8192, 8192, 8192, session);
///     let mut output = [0_u8; 1];
///     assert!(Pin::new(&mut port).read_at_async(0, &mut output).await.is_ok());
/// }
/// ```
#[cfg(feature = "async")]
#[must_use]
pub struct AsyncManagedReadPort {
    /// Open-time identity and metadata for the retained provider object.
    info: OpenedFileInfo,
    /// Readable length of that same open object.
    len: u64,
    /// Maximum bytes passed to one provider call.
    max_segment: usize,
    /// Maximum successful bytes across all reads.
    max_total: u64,
    /// Successful bytes already consumed from the cumulative budget.
    consumed: u64,
    /// Pinned session retaining the same object described by `info` and `len`.
    session: std::pin::Pin<Box<dyn crate::spi::AsyncRandomAccessReadSession>>,
}

#[cfg(feature = "async")]
impl AsyncManagedReadPort {
    /// Creates an asynchronous port bound to an already-open provider session.
    ///
    /// # Parameters
    /// - `info`: Metadata observed when opening the retained object.
    /// - `len`: Readable length of the same object retained by `session`.
    /// - `max_segment`: Maximum bytes sent to one provider call.
    /// - `max_total`: Cumulative successful-byte limit.
    /// - `session`: Pinned, already-open asynchronous provider session.
    ///
    /// # Returns
    /// An asynchronous bounded control port.
    pub fn new(
        info: OpenedFileInfo,
        len: u64,
        max_segment: usize,
        max_total: u64,
        session: std::pin::Pin<Box<dyn crate::spi::AsyncRandomAccessReadSession>>,
    ) -> Self {
        Self {
            info,
            len,
            max_segment,
            max_total,
            consumed: 0,
            session,
        }
    }

    /// Asynchronously reads one segment from the retained opened object.
    ///
    /// A canceled pending read leaves the session available for a later call
    /// at the same offset. The caller must discard the output buffer when this
    /// future is dropped or returns an error. Repeated successful ranges count
    /// again toward `max_total`.
    ///
    /// # Parameters
    /// - `self`: Pinned mutable access to this read port.
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
    pub async fn read_at_async(self: std::pin::Pin<&mut Self>, offset: u64, output: &mut [u8]) -> FsResult<usize> {
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
        let this = self.get_mut();
        if offset >= this.len {
            return Ok(0);
        }
        if this.max_segment == 0 {
            return Err(FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "managed read segment limit is zero",
            )
            .with_path(this.info.path().clone()));
        }
        let remaining_budget = this.max_total.saturating_sub(this.consumed);
        if remaining_budget == 0 {
            return Err(FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "managed read byte budget exhausted",
            )
            .with_path(this.info.path().clone()));
        }
        let count = output
            .len()
            .min(this.max_segment)
            .min(usize::try_from(this.len - offset).unwrap_or(usize::MAX))
            .min(usize::try_from(remaining_budget).unwrap_or(usize::MAX));
        if count == 0 {
            return Ok(0);
        }
        let path = this.info.path().clone();
        let n = this
            .session
            .as_mut()
            .read_at_async(offset, &mut output[..count])
            .await
            .map_err(|error| {
                error
                    .with_missing_context(&path, None, "managed-read")
                    .with_operation(FsOperation::Read)
            })?;
        if n > count {
            return Err(FsError::new(
                FsErrorKind::ProviderContractViolation,
                FsOperation::Read,
                "provider returned more bytes than supplied buffer",
            )
            .with_path(path));
        }
        this.consumed += u64::try_from(n).expect("read count is bounded by a usize buffer");
        Ok(n)
    }
}
