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

#[derive(Debug)]
pub struct CopyRecovery {
    pub failure: CopyFailure,
    pub cleanup_error: Option<FsError>,
}

pub fn copy_report(filesystem: &FileSystem, source: &Path, target: &Path) -> Result<CopyOutcome, CopyRecovery> {
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
