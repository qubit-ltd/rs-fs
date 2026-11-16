use std::collections::BTreeSet;
use std::time::Instant;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;

/// Resource limits used while validating a staged directory tree.
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
    pub fn validate_entry(
        &self,
        relative_path: &str,
        normalized_path: &str,
        is_symlink: bool,
        entries_seen: usize,
        content_bytes_seen: u64,
        measured_content_bytes: u64,
        normalized_paths: &mut BTreeSet<String>,
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
        if entries_seen.checked_add(1).is_none_or(|count| count > self.max_entries) {
            return Err(tree_validation_error("tree exceeds entry limit"));
        }
        if content_bytes_seen
            .checked_add(measured_content_bytes)
            .is_none_or(|total| total > self.max_content_bytes)
        {
            return Err(tree_validation_error("tree exceeds content byte limit"));
        }
        if normalized_path.is_empty() || normalized_paths.contains(normalized_path) {
            return Err(tree_validation_error(
                "tree entry conflicts after provider normalization",
            ));
        }
        normalized_paths.insert(normalized_path.to_owned());
        Ok(())
    }
}

fn is_safe_relative_path(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.contains(':')
        && path
            .split('/')
            .all(|part| !part.is_empty() && part != "." && part != "..")
}

fn tree_validation_error(message: &'static str) -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::ValidateProviderOutcome,
        message,
    )
}
