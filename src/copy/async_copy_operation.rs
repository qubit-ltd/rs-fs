// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Owning asynchronous copy operation with cancellation-safe state tracking.

use crate::spi::ResolvedCopyOptions;
use crate::{
    AsyncCopyFailure,
    AsyncCopyOperationState,
    AsyncFileSystem,
    AsyncFileWriter,
    CopyFailureState,
    CopyOptions,
    CopyOutcome,
    CopyStats,
    FsError,
    FsErrorKind,
    FsOperation,
    Path,
};

/// An owning copy request whose recovery writer remains accessible after
/// failure.
pub struct AsyncCopyOperation {
    pub(crate) file_system: AsyncFileSystem,
    source: Path,
    target: Path,
    options: ResolvedCopyOptions,
    state: AsyncCopyOperationState,
    writer: Option<AsyncFileWriter>,
}

impl AsyncCopyOperation {
    /// Builds a preflight-validated operation without provider I/O.
    pub(crate) fn new(
        file_system: AsyncFileSystem,
        source: Path,
        target: Path,
        options: CopyOptions,
    ) -> Self {
        Self {
            file_system,
            source,
            target,
            options: ResolvedCopyOptions::new(options),
            state: AsyncCopyOperationState::Ready,
            writer: None,
        }
    }

    /// Returns the immutable source path.
    #[must_use]
    pub const fn source(&self) -> &Path {
        &self.source
    }

    /// Returns the immutable destination path.
    #[must_use]
    pub const fn target(&self) -> &Path {
        &self.target
    }

    /// Returns the current operation lifecycle state.
    #[must_use]
    pub const fn state(&self) -> AsyncCopyOperationState {
        self.state
    }

    /// Returns whether a recovery writer is retained by this operation.
    #[must_use]
    pub const fn has_recovery_writer(&self) -> bool {
        self.writer.is_some()
    }

    /// Borrows the retained recovery writer, if one exists.
    pub fn recovery_writer(&mut self) -> Option<&mut AsyncFileWriter> {
        self.writer.as_mut()
    }

    /// Takes ownership of the retained recovery writer, if one exists.
    pub fn take_recovery_writer(&mut self) -> Option<AsyncFileWriter> {
        self.writer.take()
    }

    /// Executes the operation exactly once.
    ///
    /// The operation becomes running only when this future is first polled.
    /// Dropping a polled pending future records indeterminate state and does
    /// not invoke provider cleanup or start any additional I/O.
    pub async fn execute(&mut self) -> Result<CopyOutcome, AsyncCopyFailure> {
        if self.state != AsyncCopyOperationState::Ready {
            return Err(invalid_state_failure(&self.source));
        }
        let Self {
            file_system,
            source,
            target,
            options,
            state,
            writer,
        } = self;
        let mut guard = CopyCancellationGuard::start(state);
        let result = file_system
            .execute_copy(source, target, options, writer)
            .await;
        guard.finish(&result);
        result
    }
}

/// Marks a polled operation indeterminate if cancellation interrupts provider
/// I/O.
struct CopyCancellationGuard<'a> {
    state: &'a mut AsyncCopyOperationState,
    finished: bool,
}

impl<'a> CopyCancellationGuard<'a> {
    /// Starts tracking cancellation immediately before the first provider
    /// await.
    fn start(state: &'a mut AsyncCopyOperationState) -> Self {
        *state = AsyncCopyOperationState::Running;
        Self {
            state,
            finished: false,
        }
    }

    /// Records the completed result and disables cancellation handling.
    fn finish(&mut self, result: &Result<CopyOutcome, AsyncCopyFailure>) {
        *self.state = match result {
            Ok(_) => AsyncCopyOperationState::Completed,
            Err(failure) => AsyncCopyOperationState::Failed(failure.state()),
        };
        self.finished = true;
    }
}

impl Drop for CopyCancellationGuard<'_> {
    /// Records only local state; drop never calls a provider.
    fn drop(&mut self) {
        if !self.finished && *self.state == AsyncCopyOperationState::Running {
            *self.state = AsyncCopyOperationState::Failed(
                CopyFailureState::Indeterminate,
            );
        }
    }
}

/// Builds the stable failure used for an invalid execute retry.
fn invalid_state_failure(path: &Path) -> AsyncCopyFailure {
    AsyncCopyFailure::new(
        FsError::new(
            FsErrorKind::InvalidState,
            FsOperation::Copy,
            "copy operation cannot execute in its current state",
        )
        .with_path(path.clone()),
        CopyFailureState::Unchanged,
        CopyStats::default(),
        None,
    )
}
