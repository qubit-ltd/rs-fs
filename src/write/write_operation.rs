// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through public
// facade tests.
//! Synchronous aggregate write operation implementation.

use qubit_io::Output;

use crate::FileSystem;
use crate::error::FsError;
use crate::error::FsOperation;
use crate::error::OpenFailureStage;
use crate::metadata::WriteOutcome;
use crate::path::Path;
use crate::write::WriteAllFailure;
use crate::write::WriteFailureState;
use crate::write::WriteOptions;
use crate::write::WriterRecovery;
use crate::write::internal::open_failure_state;

/// Executes aggregate synchronous write operations for one facade.
pub(crate) struct WriteOperation<'a> {
    /// Facade that opens and contextualizes the writer.
    filesystem: &'a FileSystem,
}

impl<'a> WriteOperation<'a> {
    /// Creates a write operation bound to `filesystem`.
    #[inline]
    pub(crate) const fn new(filesystem: &'a FileSystem) -> Self {
        Self { filesystem }
    }

    /// Writes all bytes and retains the writer if transfer or commit fails.
    pub(crate) fn write_all(
        &self,
        path: &Path,
        bytes: &[u8],
        options: WriteOptions,
    ) -> Result<WriteOutcome, WriteAllFailure> {
        if let Err(error) = self
            .filesystem
            .properties()
            .limits()
            .validate_write_size(path, bytes.len())
        {
            return Err(WriteAllFailure::new(
                self.filesystem.core().enrich(error, Some(path), FsOperation::Write),
                WriteFailureState::NotPublished,
                0,
                None,
            ));
        }
        let mut writer = self.filesystem.open_writer(path, options).map_err(|failure| {
            let (error, stage, recovery) = failure.into_parts();
            let state = match stage {
                OpenFailureStage::Preflight => WriteFailureState::NotPublished,
                OpenFailureStage::ProviderOpen => open_failure_state(&error),
                OpenFailureStage::OutcomeValidation => WriteFailureState::Indeterminate,
            };
            WriteAllFailure::new(error, state, 0, recovery.map(WriterRecovery::Rejected))
        })?;
        if let Err(error) = Output::write_fully(&mut writer, bytes).and_then(|_| Output::flush(&mut writer)) {
            let error = FsError::from_stream_io(error, FsOperation::Write, path)
                .with_provider(self.filesystem.properties().info().provider_id());
            let state = if error.has_indeterminate_effect() {
                WriteFailureState::Indeterminate
            } else {
                writer.state().publication_failure_state()
            };
            return Err(WriteAllFailure::new(
                error,
                state,
                writer.written_bytes(),
                Some(WriterRecovery::Opened(Box::new(writer))),
            ));
        }
        writer.commit().map_err(|failure| {
            let (error, state) = failure.into_parts();
            WriteAllFailure::new(
                error,
                state,
                writer.written_bytes(),
                Some(WriterRecovery::Opened(Box::new(writer))),
            )
        })
    }
}
