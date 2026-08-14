// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Shared byte-budget adapters for filesystem facades.

use qubit_budget::BudgetError;
use qubit_budget::ResourceBudget;

use crate::FsError;
use crate::FsErrorKind;
use crate::FsOperation;
use crate::Path;

/// Resources counted by the filesystem facade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileSystemResource {
    /// Bytes read from a source.
    ReadBytes,
    /// Bytes accepted by a destination writer.
    WriteBytes,
}

/// A budget that counts filesystem bytes.
pub(crate) type ByteBudget = ResourceBudget<FileSystemResource, u64>;

/// Creates a byte budget with the supplied inclusive limit.
#[inline]
pub(crate) fn byte_budget(
    resource: FileSystemResource,
    maximum: u64,
) -> ByteBudget {
    ResourceBudget::new(resource, maximum)
}

/// Converts a platform-sized I/O count to the budget quantity type.
#[inline]
pub(crate) fn quantity_from_usize(
    value: usize,
    operation: FsOperation,
    path: &Path,
    provider: &str,
) -> Result<u64, FsError> {
    u64::try_from(value).map_err(|error| {
        FsError::with_source(
            FsErrorKind::ResourceLimitExceeded,
            operation,
            "I/O byte count cannot be represented by the resource budget",
            error,
        )
        .with_path(path.clone())
        .with_provider(provider)
    })
}

/// Converts a budget failure into a contextual filesystem error.
#[inline]
pub(crate) fn budget_error(
    error: BudgetError<FileSystemResource, u64>,
    operation: FsOperation,
    path: &Path,
    provider: &str,
    message: &'static str,
) -> FsError {
    FsError::with_source(
        FsErrorKind::ResourceLimitExceeded,
        operation,
        message,
        error,
    )
    .with_path(path.clone())
    .with_provider(provider)
}
