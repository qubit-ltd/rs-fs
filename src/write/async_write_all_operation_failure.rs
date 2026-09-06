//! Failure for an owning asynchronous whole-file write.
use std::error::Error;
use std::fmt::Display;
use std::fmt::Formatter;

use crate::error::FsError;
use crate::write::WriteFailureState;
/// Error returned by an owning asynchronous whole-file write.
pub struct AsyncWriteAllOperationFailure {
    error: FsError,
    state: WriteFailureState,
    written_bytes: u64,
}
impl AsyncWriteAllOperationFailure {
    /// Creates a failure with its recovery facts.
    pub(crate) fn new(error: FsError, state: WriteFailureState, written_bytes: u64) -> Self {
        Self {
            error,
            state,
            written_bytes,
        }
    }
    /// Returns the contextual filesystem error.
    #[must_use]
    pub const fn error(&self) -> &FsError {
        &self.error
    }
    /// Returns the provider-confirmed publication state.
    #[must_use]
    pub const fn state(&self) -> WriteFailureState {
        self.state
    }
    /// Returns bytes accepted before failure or cancellation.
    #[must_use]
    pub const fn written_bytes(&self) -> u64 {
        self.written_bytes
    }
    /// Consumes the failure and returns its filesystem error.
    #[must_use]
    pub fn into_error(self) -> FsError {
        self.error
    }
    /// Consumes the failure and returns all recovery facts.
    #[must_use]
    pub fn into_parts(self) -> (FsError, WriteFailureState, u64) {
        (self.error, self.state, self.written_bytes)
    }
}
impl Display for AsyncWriteAllOperationFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::fmt::Debug for AsyncWriteAllOperationFailure {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AsyncWriteAllOperationFailure")
            .field("error", &self.error)
            .field("state", &self.state)
            .field("written_bytes", &self.written_bytes)
            .finish()
    }
}
impl Error for AsyncWriteAllOperationFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}
