// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::time::Instant;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::TreeEntryValidationState;

/// Resource limits used while validating a staged directory tree.
///
/// # Examples
///
/// ```rust
/// use std::collections::BTreeSet;
///
/// use qubit_fs::metadata::TreeValidationOptions;
///
/// let limits = TreeValidationOptions::default();
/// let mut normalized_paths = BTreeSet::new();
/// limits.validate_entry(
///     "docs/readme.md",
///     "docs/readme.md",
///     false,
///     &mut qubit_fs::metadata::TreeEntryValidationState {
///         entries_seen: 0,
///         content_bytes_seen: 0,
///         measured_content_bytes: 12,
///         normalized_paths: &mut normalized_paths,
///     },
/// )?;
/// assert_eq!(normalized_paths.len(), 1);
///
/// # Ok::<(), qubit_fs::error::FsError>(())
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TreeValidationOptions {
    /// Maximum number of entries.
    pub max_entries: usize,
    /// Maximum relative path depth.
    pub max_depth: usize,
    /// Maximum aggregate file content bytes.
    pub max_content_bytes: u64,
}

impl Default for TreeValidationOptions {
    /// Uses limits of 100,000 entries, depth 128, and 1 TiB of aggregate
    /// content.
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_depth: 128,
            max_content_bytes: 1 << 40,
        }
    }
}

impl TreeValidationOptions {
    /// Maximum records accepted in one tree-validation batch.
    pub const MAX_BATCH_RECORDS: usize = 128;
    /// Maximum encoded record bytes accepted in one tree-validation batch.
    pub const MAX_BATCH_BYTES: usize = 256 * 1024;

    /// Checks a validation deadline and an independent stop request.
    ///
    /// # Parameters
    ///
    /// - `now`: Current time used to compare the deadline.
    /// - `deadline`: Optional instant at which validation must stop.
    /// - `stop_requested`: Whether an independent cancellation was requested.
    ///
    /// # Errors
    ///
    /// Returns a provider-contract error when cancellation is requested or
    /// `now` has reached or passed `deadline`.
    pub fn check_active(&self, now: Instant, deadline: Option<Instant>, stop_requested: bool) -> FsResult<()> {
        if stop_requested {
            return Err(tree_validation_error("tree validation was stopped"));
        }
        if deadline.is_some_and(|deadline| now >= deadline) {
            return Err(tree_validation_error("tree validation deadline expired"));
        }
        Ok(())
    }

    /// Checks the record count and encoded byte size of one validation batch.
    ///
    /// Record sizes must be measured from the encoded records, not estimated
    /// from entry metadata. A single record larger than the batch limit is
    /// rejected even when it is the only record.
    ///
    /// # Parameters
    ///
    /// - `record_sizes`: Encoded byte length of each record in the batch.
    ///
    /// # Errors
    ///
    /// Returns a provider-contract error when the batch exceeds the record or
    /// byte limit, one record exceeds the byte limit, or byte-count addition
    /// overflows.
    pub fn validate_batch(&self, record_sizes: &[usize]) -> FsResult<()> {
        if record_sizes.len() > Self::MAX_BATCH_RECORDS {
            return Err(tree_validation_error("tree validation batch exceeds record limit"));
        }
        let mut total = 0_usize;
        for size in record_sizes {
            if *size > Self::MAX_BATCH_BYTES {
                return Err(tree_validation_error("tree validation record exceeds byte limit"));
            }
            total = total
                .checked_add(*size)
                .ok_or_else(|| tree_validation_error("tree validation batch byte count overflowed"))?;
            if total > Self::MAX_BATCH_BYTES {
                return Err(tree_validation_error("tree validation batch exceeds byte limit"));
            }
        }
        Ok(())
    }

    /// Validates one relative tree entry and accounts for measured content.
    ///
    /// `measured_content_bytes` must be the actual number of bytes read so
    /// far, including when the provider did not know the entry length in
    /// advance. `normalized_path` is the provider's canonical relative path;
    /// it is inserted only after every check succeeds.
    ///
    /// # Parameters
    ///
    /// - `relative_path`: Provider-reported path before normalization.
    /// - `normalized_path`: Canonical relative path used for duplicate checks.
    /// - `is_symlink`: Whether the provider reports a symbolic link.
    /// - `state`: Counters and canonical-path set updated after success.
    ///
    /// # Errors
    ///
    /// Returns a provider-contract error for symbolic links, unsafe paths,
    /// depth/entry/content limits, arithmetic overflow, or a normalized path
    /// that is empty or already present. The set is unchanged on error.
    pub fn validate_entry(
        &self,
        relative_path: &str,
        normalized_path: &str,
        is_symlink: bool,
        state: &mut TreeEntryValidationState<'_>,
    ) -> FsResult<()> {
        if is_symlink {
            return Err(tree_validation_error("tree validation rejects symbolic links"));
        }
        if !is_safe_relative_path(relative_path) || !is_safe_relative_path(normalized_path) {
            return Err(tree_validation_error("tree entry path is absolute or escapes its root"));
        }
        if relative_path.split('/').count() > self.max_depth {
            return Err(tree_validation_error("tree entry exceeds depth limit"));
        }
        if state
            .entries_seen
            .checked_add(1)
            .is_none_or(|count| count > self.max_entries)
        {
            return Err(tree_validation_error("tree exceeds entry limit"));
        }
        if state
            .content_bytes_seen
            .checked_add(state.measured_content_bytes)
            .is_none_or(|total| total > self.max_content_bytes)
        {
            return Err(tree_validation_error("tree exceeds content byte limit"));
        }
        if normalized_path.is_empty() || state.normalized_paths.contains(normalized_path) {
            return Err(tree_validation_error(
                "tree entry conflicts after provider normalization",
            ));
        }
        state.normalized_paths.insert(normalized_path.to_owned());
        Ok(())
    }
}

/// Reports whether a path is a nonempty relative path without traversal
/// components.
///
/// Absolute paths, backslashes, colons, empty components, `.` and `..` are
/// rejected.
///
/// # Parameters
/// - `path`: Relative path reported by the provider.
///
/// # Returns
/// `true` when every path component is nonempty and avoids traversal syntax.
fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

/// Creates a provider-contract error for a failed staged-tree validation.
///
/// The error is attributed to validating the provider's reported outcome.
///
/// # Parameters
/// - `message`: Static explanation of the provider contract violation.
///
/// # Returns
/// A validation error carrying the supplied diagnostic.
fn tree_validation_error(message: &'static str) -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::ValidateProviderOutcome,
        message,
    )
}
