//! Object-safe owned read provider protocol.
use crate::FsOperationFailure;
use crate::execution::{FsOwnedReadRequest, FsPrepareFailure};
use crate::{
    error::{FsError, FsErrorKind, FsOperation, FsResult},
    metadata::OpenedFileInfo,
    spi::RandomAccessReadSession,
};
use std::sync::{Arc, Mutex};

/// Thread-safe control port for bounded reads from one opened object.
///
/// Clones share one serialized provider session and one cumulative byte
/// budget. The session is never reopened by path, so replacing a path after
/// constructing this port does not redirect reads to a different object.
pub struct ManagedReadPort {
    state: Arc<Mutex<ManagedReadState>>,
}

struct ManagedReadState {
    info: OpenedFileInfo,
    len: u64,
    max_segment: usize,
    max_total: u64,
    consumed: u64,
    session: Box<dyn RandomAccessReadSession>,
}

impl Clone for ManagedReadPort {
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
    pub fn info(&self) -> FsResult<OpenedFileInfo> {
        self.state
            .lock()
            .map(|state| state.info.clone())
            .map_err(|_| poisoned_read_error())
    }

    /// Returns the open-time readable length; it does not imply a snapshot.
    pub fn len(&self) -> FsResult<u64> {
        self.state
            .lock()
            .map(|state| state.len)
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
        let version = self
            .info()?
            .metadata()
            .and_then(|metadata| metadata.etag().cloned());
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(capacity).map_err(|error| {
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
    pub fn read_prefix(&self, max_bytes: usize) -> FsResult<crate::read::PrefixReadOutcome> {
        let info = self.info()?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(max_bytes).map_err(|error| {
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
    /// # Errors
    /// Returns typed filesystem errors for invalid offsets, exhausted byte
    /// budget, provider errors, or provider over-reporting.
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
        let n = state
            .session
            .read_at(offset, &mut output[..count])
            .map_err(|error| {
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

fn poisoned_read_error() -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::Read,
        "managed read session lock was poisoned",
    )
}

/// Mutable asynchronous control port for bounded reads from one opened object.
///
/// Calls borrow the port mutably for the entire operation, which serializes
/// reads without retaining a borrowed SPI future or requiring a runtime.
#[cfg(feature = "async")]
pub struct AsyncManagedReadPort {
    info: OpenedFileInfo,
    len: u64,
    max_segment: usize,
    max_total: u64,
    consumed: u64,
    session: std::pin::Pin<Box<dyn crate::spi::AsyncRandomAccessReadSession>>,
}

#[cfg(feature = "async")]
impl AsyncManagedReadPort {
    /// Creates an asynchronous port bound to an already-open provider session.
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
    pub async fn read_at_async(
        self: std::pin::Pin<&mut Self>,
        offset: u64,
        output: &mut [u8],
    ) -> FsResult<usize> {
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
/// Prepared read accepted by the managed operation framework.
pub trait PreparedFsRead:
    qubit_operation::PreparedExternalOperation<Vec<u8>, FsOperationFailure>
{
    /// Returns the original request while the prepared operation remains unclaimed.
    fn take_unclaimed_request(&self) -> Option<FsOwnedReadRequest>;
}
/// Object-safe provider preparation interface for owned read requests.
pub trait ManagedFsReadSpi: Send + Sync {
    /// Reserves provider resources and prepares a read without starting I/O.
    fn prepare_read_all(
        &self,
        request: FsOwnedReadRequest,
    ) -> Result<Arc<dyn PreparedFsRead>, FsPrepareFailure<FsOwnedReadRequest>>;
}
