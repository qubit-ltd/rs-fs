// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Asynchronous filesystem provider contract and error classification.

use qubit_spi::AsyncProviderDefinition;
use qubit_spi::error::ProviderError;

use crate::{
    FileSystemSpec,
    FsError,
    FsErrorKind,
};

/// Metadata-bearing asynchronous filesystem provider.
pub trait AsyncFileSystemProvider:
    AsyncProviderDefinition<FileSystemSpec>
{
}

impl<T> AsyncFileSystemProvider for T where
    T: AsyncProviderDefinition<FileSystemSpec> + ?Sized
{
}

/// Converts a filesystem creation failure into an SPI leaf provider failure.
///
/// # Arguments
///
/// * `error` - Filesystem error returned while creating an async filesystem.
///
/// # Returns
///
/// A classified provider error retaining the original filesystem error as its
/// source.
#[must_use]
pub fn map_async_provider_error(error: FsError) -> ProviderError {
    let reason = format!("asynchronous filesystem provider failed: {error}");
    match error.kind() {
        FsErrorKind::ProviderUnavailable => {
            ProviderError::unavailable_with_source(reason, error)
        }
        FsErrorKind::UnsupportedOperation
        | FsErrorKind::UnsupportedCapability
        | FsErrorKind::RequirementNotMet => {
            ProviderError::unsupported_with_source(reason, error)
        }
        FsErrorKind::InvalidUri
        | FsErrorKind::InvalidPath
        | FsErrorKind::InvalidOptions => {
            ProviderError::invalid_configuration_with_source(reason, error)
        }
        _ => ProviderError::initialization_failed_with_source(reason, error),
    }
}
