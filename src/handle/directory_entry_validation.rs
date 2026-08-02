// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Shared validation for provider directory entries.

use crate::{
    DirEntry,
    FileSystemLimits,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    ListOptions,
    Path,
    PathSemantics,
};

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
        .validate_path(
            &entry.path,
            semantics,
            FsOperation::ValidateProviderOutcome,
        )
        .is_err()
    {
        return Err(contract_error(
            &entry.path,
            "provider returned a directory entry outside filesystem path limits",
        ));
    }
    let expected_name = entry
        .path
        .components()
        .last()
        .map(|component| component.to_string())
        .unwrap_or_default();
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
    if relative_path(root, &entry.path).is_none() {
        return Err(contract_error(
            root,
            "provider returned directory entry outside requested root",
        ));
    }
    Ok(())
}

/// Checks whether one validated entry is selected by listing options.
pub(crate) fn matches_options(
    entry: &DirEntry,
    root: &Path,
    options: &ListOptions,
) -> bool {
    let Some(relative) = relative_path(root, &entry.path) else {
        return false;
    };
    if !options.recursive()
        && options.prefix().is_none()
        && relative.contains('/')
    {
        return false;
    }
    if options.include_metadata() && entry.metadata.is_none() {
        return false;
    }
    options.prefix().is_none_or(|prefix| {
        relative == prefix
            || relative
                .strip_prefix(prefix)
                .is_some_and(|remaining| remaining.starts_with('/'))
    })
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

/// Builds a provider-contract error for one invalid entry identity.
fn contract_error(path: &Path, message: &'static str) -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::ValidateProviderOutcome,
        message,
    )
    .with_path(path.clone())
}
