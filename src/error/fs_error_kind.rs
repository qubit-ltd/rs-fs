// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Filesystem error categories.

/// Provider-neutral filesystem error category.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FsErrorKind {
    /// The requested resource does not exist.
    NotFound,
    /// The target resource already exists.
    AlreadyExists,
    /// A path component expected to be a directory is not a directory.
    NotDirectory,
    /// The requested operation expected a file but found a directory.
    IsDirectory,
    /// A provider-local path is invalid for the selected filesystem.
    InvalidPath,
    /// A filesystem URI is malformed or violates the credential boundary.
    InvalidUri,
    /// Operation options or provider construction input are inconsistent.
    InvalidOptions,
    /// A provider returned a value that violates the SPI contract.
    ProviderContractViolation,
    /// The current credentials do not grant the requested operation.
    PermissionDenied,
    /// Authentication failed before authorization could be evaluated.
    AuthenticationFailed,
    /// The selected provider is unavailable in the current environment.
    ProviderUnavailable,
    /// The filesystem model does not support the requested operation.
    UnsupportedOperation,
    /// A specific stable filesystem capability is not supported.
    UnsupportedCapability,
    /// The operation exists but cannot satisfy a required semantic guarantee.
    RequirementNotMet,
    /// The handle or resource is not in a state that permits the operation.
    InvalidState,
    /// Side effects may have occurred, but their final state is unknown.
    Indeterminate,
    /// The operation was cancelled before a definitive result was produced.
    Cancelled,
    /// The operation conflicts with current filesystem state.
    Conflict,
    /// A conditional operation failed its precondition.
    PreconditionFailed,
    /// The operation timed out.
    Timeout,
    /// The operation was interrupted.
    Interrupted,
    /// A quota, capacity, or storage limit was exceeded.
    QuotaExceeded,
    /// A deterministic caller or provider resource bound was exceeded.
    ResourceLimitExceeded,
    /// Stored or transferred data failed integrity validation.
    DataCorruption,
    /// A lower-level local or remote I/O error occurred.
    Io,
    /// An error occurred that does not fit a more specific category.
    Other,
}
