/*******************************************************************************
 *
 *    Copyright (c) 2026 Haixing Hu.
 *
 *    SPDX-License-Identifier: Apache-2.0
 *
 *    Licensed under the Apache License, Version 2.0.
 *
 ******************************************************************************/
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
    /// A path or URI is invalid for the selected filesystem.
    InvalidPath,
    /// The current credentials do not grant the requested operation.
    PermissionDenied,
    /// Authentication failed before authorization could be evaluated.
    AuthenticationFailed,
    /// The selected provider is unavailable in the current environment.
    ProviderUnavailable,
    /// The filesystem model does not support the requested operation.
    UnsupportedOperation,
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
    /// Stored or transferred data failed integrity validation.
    DataCorruption,
    /// A lower-level local or remote I/O error occurred.
    Io,
    /// An error occurred that does not fit a more specific category.
    Other,
}
