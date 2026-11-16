// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Shared fixtures for public random-access contract tests.
use qubit_fs::Path;
use qubit_fs::metadata::FileKind;
use qubit_fs::metadata::FileMetadata;
use qubit_fs::metadata::FileSystemId;
use qubit_fs::metadata::OpenedFileInfo;

/// Builds metadata representing the file and fixed length of one opened object.
pub(crate) fn opened_info(provider_id: &str, path: &Path, len: u64) -> OpenedFileInfo {
    OpenedFileInfo::new(FileSystemId::new(provider_id).expect("provider id"), path.clone())
        .with_metadata(FileMetadata::new(FileKind::File).with_len(Some(len)))
}
