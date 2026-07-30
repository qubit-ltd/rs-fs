// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
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

use super::internal::CopyCancellationGuard;

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
    ///
    /// # Returns
    /// The source path captured when the operation was created.
    #[inline(always)]
    #[must_use]
    pub const fn source(&self) -> &Path {
        &self.source
    }

    /// Returns the immutable destination path.
    #[inline(always)]
    #[must_use]
    pub const fn target(&self) -> &Path {
        &self.target
    }

    /// Returns the current operation lifecycle state.
    #[inline(always)]
    #[must_use]
    pub const fn state(&self) -> AsyncCopyOperationState {
        self.state
    }

    /// Returns whether a recovery writer is retained by this operation.
    #[inline(always)]
    #[must_use]
    pub const fn has_recovery_writer(&self) -> bool {
        self.writer.is_some()
    }

    /// Borrows the retained recovery writer, if one exists.
    #[inline(always)]
    pub fn recovery_writer(&mut self) -> Option<&mut AsyncFileWriter> {
        self.writer.as_mut()
    }

    /// Takes ownership of the retained recovery writer, if one exists.
    #[inline(always)]
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
        let mut guard = CopyCancellationGuard::start(state, writer);
        let result = file_system
            .execute_copy(source, target, options, guard.writer_mut())
            .await;
        guard.finish(&result);
        result
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
    )
}
