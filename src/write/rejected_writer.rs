// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Explicit cleanup authority for a rejected write session.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::error::RecoveryCleanupState;
use crate::facade::internal::RecoveryCleanupGuard;
use crate::path::Path;
use crate::spi::FileWriterSpi;
use crate::write::WriteAbortOutcome;

/// Isolated cleanup ownership after a provider returned an invalid identity.
///
/// This handle cannot write or publish. Cleanup acts only on resources owned by
/// its session, never on a diagnostic path. Drop performs no explicit cleanup
/// or cancellation hook; retain this handle until cleanup is confirmed.
///
/// ```compile_fail
/// use qubit_fs::write::RejectedWriter;
/// fn cannot_publish(mut recovery: RejectedWriter) {
///     recovery.commit();
/// }
/// ```
#[must_use = "explicitly clean or retain the isolated recovery session"]
pub struct RejectedWriter {
    /// The actual provider session, retained independently of cleanup futures.
    session: Box<dyn FileWriterSpi>,
    /// Last observed cleanup state, never a publication snapshot.
    state: RecoveryCleanupState,
    /// Trusted configured provider identifier.
    provider: Box<str>,
    /// Requested path, never the unvalidated provider identity.
    path: Option<Path>,
}
impl RejectedWriter {
    /// Takes ownership of a rejected session and trusted request context.
    pub(crate) fn new(session: Box<dyn FileWriterSpi>, provider: &str, path: Option<Path>) -> Self {
        Self {
            session,
            state: RecoveryCleanupState::Pending,
            provider: provider.into(),
            path,
        }
    }
    /// Returns cleanup progress; errors and cancellation do not release
    /// ownership.
    pub const fn cleanup_state(&self) -> RecoveryCleanupState {
        self.state
    }

    /// Explicitly cleans resources actually owned by this isolated session.
    ///
    /// A failed attempt retains the session for explicit retry or
    /// reconciliation. This synchronous method may block in provider I/O.
    ///
    /// # Errors
    /// Returns InvalidState after confirmed cleanup, or the contextual provider
    /// error without replacing the original opening failure.
    pub fn abort(&mut self) -> FsResult<WriteAbortOutcome> {
        if self.state == RecoveryCleanupState::Completed {
            return Err(self.contextual_error(FsError::new(
                FsErrorKind::InvalidState,
                FsOperation::AbortWriter,
                "isolated session cleanup already completed",
            )));
        }
        let mut guard = RecoveryCleanupGuard::start(&mut self.state);
        let result = self.session.abort();
        guard.finish(matches!(
            &result,
            Ok(WriteAbortOutcome::NotPublished | WriteAbortOutcome::Published)
        ));
        drop(guard);
        result.map_err(|error| self.contextual_error(error))
    }
    /// Adds only trusted request context to a cleanup error.
    fn contextual_error(&self, error: FsError) -> FsError {
        error.with_trusted_cleanup_context(FsOperation::AbortWriter, self.path.as_ref(), &self.provider)
    }
}
impl Debug for RejectedWriter {
    /// Omits the session and unvalidated provider identity.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("RejectedWriter")
            .field("cleanup_state", &self.state)
            .finish_non_exhaustive()
    }
}
