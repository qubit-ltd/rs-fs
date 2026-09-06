// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Shared validation for provider directory entries.

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::DirEntry;
use crate::metadata::FileSystemLimits;
use crate::path::Path;
use crate::path::PathSemantics;

/// Validates provider-controlled identity fields before applying list policy.
pub(crate) fn validate_entry(
    entry: &DirEntry,
    root: &Path,
    semantics: PathSemantics,
    limits: FileSystemLimits,
) -> FsResult<()> {
    if entry.path.semantics() != semantics {
        return Err(contract_error(
            &entry.path,
            "provider returned a directory entry with foreign path semantics",
        ));
    }
    if limits
        .validate_path(&entry.path, semantics, FsOperation::ValidateProviderOutcome)
        .is_err()
    {
        return Err(contract_error(
            &entry.path,
            "provider returned a directory entry outside filesystem path limits",
        ));
    }
    let expected_name = entry.path.file_name().unwrap_or_default();
    if entry.name != expected_name {
        return Err(contract_error(
            &entry.path,
            "provider returned a directory entry with an inconsistent name",
        ));
    }
    if entry
        .metadata
        .as_ref()
        .is_some_and(|metadata| metadata.kind() != &entry.kind)
    {
        return Err(contract_error(
            &entry.path,
            "provider returned directory metadata with a different kind",
        ));
    }
    if crate::directory::internal::relative_path(root, &entry.path, semantics).is_none() {
        return Err(contract_error(
            root,
            "provider returned directory entry outside requested root",
        ));
    }
    Ok(())
}

/// Checks whether one validated entry is selected by listing options.
/// Builds a stable provider-contract error for list option filtering.
pub(crate) fn option_error(root: &Path, message: &'static str) -> FsError {
    contract_error(root, message)
}

/// Returns the entry path relative to `root` when it remains in the root.
fn relative_path<'a>(root: &Path, entry: &'a Path) -> Option<&'a str> {
    if root == entry {
        Some("")
    } else if root.as_str() == "/" {
        entry.as_str().strip_prefix('/')
    } else {
        let remainder = entry.as_str().strip_prefix(root.as_str())?;
        if root.as_str().ends_with('/') {
            Some(remainder)
        } else {
            remainder.strip_prefix('/')
        }
    }
}

/// Returns the descendant depth of `entry` below `root`.
pub(crate) fn entry_depth(root: &Path, entry: &Path) -> Option<usize> {
    let relative = relative_path(root, entry)?;
    if relative.is_empty() {
        Some(0)
    } else {
        Some(relative.split('/').count())
    }
}

/// Builds a provider-contract error for one invalid entry identity.
fn contract_error(path: &Path, message: &'static str) -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::ValidateProviderOutcome,
        message,
    )
    .with_path(path.clone())
}
