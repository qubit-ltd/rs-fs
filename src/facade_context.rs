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

use crate::FileSystemCapability;
use crate::FileSystemProperties;
use crate::FsError;
use crate::FsErrorKind;
use crate::FsOperation;
use crate::FsResult;
use crate::Path;

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
        .validate_path(path, properties.info().path_semantics(), operation)
        .map_err(|error| enrich(properties, error, path, operation))
}

/// Validates the optional parent supplied for temporary resource creation.
///
/// # Parameters
///
/// - `properties`: Cached filesystem identity, path, and limit rules.
/// - `parent`: Optional logical parent path from the creation options.
///
/// # Errors
///
/// Returns an enriched `InvalidPath` or resource-limit error when `parent`
/// does not match the filesystem semantics, form, constraints, or limits.
pub(crate) fn validate_temp_parent(
    properties: &FileSystemProperties,
    parent: Option<&Path>,
) -> FsResult<()> {
    parent.map_or(Ok(()), |path| {
        validate_path(properties, path, FsOperation::CreateTemp)
    })
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
    require_optional(properties, capability, operation, Some(path))
}

/// Requires a capability while allowing pathless operations to omit fabricated
/// path context.
///
/// # Parameters
///
/// - `properties`: Cached filesystem capability snapshot.
/// - `capability`: Capability required by the operation.
/// - `operation`: Operation being preflighted.
/// - `path`: Optional path associated with the requirement.
///
/// # Errors
///
/// Returns an enriched unsupported-capability error when the snapshot does not
/// advertise `capability`.
pub(crate) fn require_optional(
    properties: &FileSystemProperties,
    capability: FileSystemCapability,
    operation: FsOperation,
    path: Option<&Path>,
) -> FsResult<()> {
    if properties.capabilities().supports(capability) {
        Ok(())
    } else {
        let error = FsError::new(
            FsErrorKind::UnsupportedCapability,
            operation,
            "filesystem capability is not supported",
        )
        .with_required_capability(capability)
        .with_missing_provider(properties.info().provider_id());
        Err(match path {
            Some(path) => error.with_path(path.clone()),
            None => error,
        })
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
    enrich_optional(properties, error, Some(path), operation)
}

/// Adds operation and provider context without inventing a path.
///
/// # Parameters
///
/// - `properties`: Cached filesystem identity and provider identifier.
/// - `error`: Provider error crossing the facade boundary.
/// - `path`: Optional logical path associated with the operation.
/// - `operation`: Public operation being returned.
///
/// # Returns
/// Error retaining provider-supplied context and filling only absent facade
/// facts.
pub(crate) fn enrich_optional(
    properties: &FileSystemProperties,
    error: FsError,
    path: Option<&Path>,
    operation: FsOperation,
) -> FsError {
    let error = error
        .with_operation(operation)
        .with_missing_provider(properties.info().provider_id());
    match path {
        Some(path) => error.with_missing_context(
            path,
            None,
            properties.info().provider_id(),
        ),
        None => error,
    }
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
