// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade tests.
//! Shared validation and error-context helpers for filesystem facades.

use crate::{
    FileSystemCapability,
    FileSystemProperties,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    Path,
};

/// Validates one logical path against an immutable provider snapshot.
///
/// # Parameters
///
/// - `properties`: Cached filesystem identity, path, and limit rules.
/// - `path`: Logical path supplied by the caller.
/// - `operation`: Operation that will consume the path.
///
/// # Errors
///
/// Returns an enriched invalid-path or resource-limit error when the path
/// semantics, form, or declared limits do not match the filesystem.
pub(crate) fn validate_path(
    properties: &FileSystemProperties,
    path: &Path,
    operation: FsOperation,
) -> FsResult<()> {
    if path.semantics() != properties.info().path_semantics() {
        return Err(FsError::invalid_path(
            operation,
            "path semantics do not match this filesystem",
        )
        .with_path(path.clone())
        .with_provider(properties.info().provider_id()));
    }
    properties
        .path_constraints()
        .validate(path)
        .map_err(|error| enrich(properties, error, path, operation))?;
    properties
        .limits()
        .validate_path(
            path,
            properties.info().path_semantics(),
            operation,
        )
        .map_err(|error| enrich(properties, error, path, operation))
}

/// Requires one capability before an operation can create provider I/O.
///
/// # Parameters
///
/// - `properties`: Cached filesystem capability snapshot.
/// - `capability`: Capability required by the operation.
/// - `operation`: Operation being preflighted.
/// - `path`: Path associated with the requirement.
///
/// # Errors
///
/// Returns an enriched unsupported-capability error when the snapshot does not
/// advertise `capability`.
pub(crate) fn require(
    properties: &FileSystemProperties,
    capability: FileSystemCapability,
    operation: FsOperation,
    path: &Path,
) -> FsResult<()> {
    if properties.capabilities().contains(capability) {
        Ok(())
    } else {
        Err(FsError::new(
            FsErrorKind::UnsupportedCapability,
            operation,
            "filesystem capability is not supported",
        )
        .with_path(path.clone())
        .with_provider(properties.info().provider_id())
        .with_required_capability(capability))
    }
}

/// Adds missing operation, path, and provider context to an error.
///
/// Existing context is preserved, so provider-specific path or provider facts
/// are not overwritten by the facade.
pub(crate) fn enrich(
    properties: &FileSystemProperties,
    error: FsError,
    path: &Path,
    operation: FsOperation,
) -> FsError {
    error.with_operation(operation).with_missing_context(
        path,
        None,
        properties.info().provider_id(),
    )
}

/// Builds a provider-contract error bound to a requested path.
///
/// # Parameters
///
/// - `properties`: Cached filesystem identity and provider identifier.
/// - `path`: Path associated with the invalid provider response.
/// - `operation`: Validation operation reported to the caller.
/// - `message`: Stable contract violation description.
pub(crate) fn contract_error(
    properties: &FileSystemProperties,
    path: &Path,
    operation: FsOperation,
    message: &str,
) -> FsError {
    FsError::new(FsErrorKind::ProviderContractViolation, operation, message)
        .with_path(path.clone())
        .with_provider(properties.info().provider_id())
}
