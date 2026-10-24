// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use qubit_fs::FileSystem;
use qubit_fs::Path;
use qubit_fs::copy::CopyFailure;
use qubit_fs::copy::CopyOptions;
use qubit_fs::copy::CopyOutcome;
use qubit_fs::error::FsError;
use qubit_fs::write::WriterRecovery;

/// Retains a copy failure and any error from aborting its writer.
#[derive(Debug)]
pub struct CopyRecovery {
    /// The copy failure, including its publication and recovery state.
    pub failure: CopyFailure,
    /// An error returned while aborting the retained writer, if any.
    pub cleanup_error: Option<FsError>,
}

/// Copies a file and retains recovery details when the operation fails.
///
/// If a writer remains available after failure, this function attempts to
/// abort it. Aborting does not guarantee that the target was rolled back.
pub fn copy_report(
    filesystem: &FileSystem,
    source: &Path,
    target: &Path,
) -> Result<CopyOutcome, CopyRecovery> {
    match filesystem.copy(source, target, CopyOptions::default()) {
        Ok(outcome) => Ok(outcome),
        Err(mut failure) => {
            // Abort handles the retained session; it does not promise target rollback.
            let cleanup_error = match failure.recovery_mut() {
                Some(WriterRecovery::Opened(writer)) => writer.abort().err(),
                Some(WriterRecovery::Rejected(writer)) => writer.abort().err(),
                None => None,
            };
            // Preserve the publication facts and writer even when cleanup fails.
            Err(CopyRecovery { failure, cleanup_error })
        }
    }
}
