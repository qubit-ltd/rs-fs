// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Explicit cleanup authority for a rejected write session.

use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::pin::Pin;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::error::RecoveryCleanupState;
use crate::facade::internal::RecoveryCleanupGuard;
use crate::path::Path;
use crate::spi::AsyncFileWriteSession;
use crate::spi::SpiFuture;
use crate::write::WriteAbortOutcome;

/// Isolated cleanup ownership after a provider returned an invalid identity.
///
/// This handle cannot write or publish. Cleanup acts only on resources owned by
/// its session, never on a diagnostic path. Drop performs no explicit cleanup
/// or cancellation hook; retain this handle until cleanup is confirmed.
///
/// ```compile_fail
/// use qubit_fs::write::RejectedAsyncWriter;
/// fn cannot_publish(mut recovery: RejectedAsyncWriter) {
///     recovery.commit();
/// }
/// ```
#[must_use = "explicitly clean or retain the isolated recovery session"]
pub struct RejectedAsyncWriter {
    /// The actual provider session, retained independently of cleanup futures.
    session: Pin<Box<dyn AsyncFileWriteSession>>,
    /// Last observed cleanup state, never a publication snapshot.
    state: RecoveryCleanupState,
    /// Trusted configured provider identifier.
    provider: Box<str>,
    /// Requested path, never the unvalidated provider identity.
    path: Option<Path>,
}
impl RejectedAsyncWriter {
    /// Takes ownership of a rejected session and trusted request context.
    pub(crate) fn new(session: Box<dyn AsyncFileWriteSession>, provider: &str, path: Option<Path>) -> Self {
        Self {
            session: Box::into_pin(session),
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
    /// reconciliation. No work occurs until polled. Dropping a polled,
    /// unfinished future records Indeterminate; dropping an unpolled future
    /// leaves the state unchanged.
    ///
    /// # Errors
    /// Returns InvalidState after confirmed cleanup, or the contextual provider
    /// error without replacing the original opening failure.
    pub fn abort_async(&mut self) -> SpiFuture<'_, FsResult<WriteAbortOutcome>> {
        Box::pin(async move {
            if self.state == RecoveryCleanupState::Completed {
                return Err(self.contextual_error(FsError::new(
                    FsErrorKind::InvalidState,
                    FsOperation::AbortWriter,
                    "isolated session cleanup already completed",
                )));
            }
            let mut guard = RecoveryCleanupGuard::start(&mut self.state);
            let result = self.session.as_mut().abort_async().await;
            guard.finish(matches!(
                &result,
                Ok(WriteAbortOutcome::NotPublished | WriteAbortOutcome::Published)
            ));
            drop(guard);
            result.map_err(|error| self.contextual_error(error))
        })
    }
    /// Adds only trusted request context to a cleanup error.
    fn contextual_error(&self, error: FsError) -> FsError {
        error.with_trusted_cleanup_context(FsOperation::AbortWriter, self.path.as_ref(), &self.provider)
    }
}
impl Debug for RejectedAsyncWriter {
    /// Omits the session and unvalidated provider identity.
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("RejectedAsyncWriter")
            .field("cleanup_state", &self.state)
            .finish_non_exhaustive()
    }
}
