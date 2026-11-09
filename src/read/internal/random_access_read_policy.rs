// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Shared validation for random-access opens and reads.
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileKind;
use crate::metadata::FileMetadata;
use crate::metadata::FileSystemCapabilities;
use crate::metadata::FileSystemLimit;
use crate::read::ReadOptions;

/// Applies the facade's shared random-access open and read policy.
///
/// Validation is centralized here so synchronous and asynchronous readers
/// reject the same requests in the same order.
pub(crate) struct RandomAccessReadPolicy;
impl RandomAccessReadPolicy {
    /// Rejects options that cannot describe a complete random-access object.
    ///
    /// Offset zero is accepted because it does not narrow the opened object;
    /// nonzero offsets and any explicit length return `InvalidOptions`.
    /// Checksum policies return `RequirementNotMet` because random-access
    /// sessions do not validate checksums.
    ///
    /// # Parameters
    /// - `options`: Read options requested for the open.
    ///
    /// # Returns
    /// `Ok(())` when options describe a complete object without checksum
    /// validation; otherwise the applicable options or requirement error.
    pub(crate) fn validate_open(options: &ReadOptions) -> FsResult<()> {
        if options.offset().is_some_and(|offset| offset != 0) || options.length().is_some() {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::OpenReader,
                "random-access sessions require a complete resource",
            ));
        }
        if options.checksum() != crate::read::ChecksumPolicy::None {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::OpenReader,
                "random-access sessions do not validate checksums",
            ));
        }
        Ok(())
    }
    /// Validates requested capabilities while treating explicit offset zero as
    /// the same unbounded request as an omitted offset.
    ///
    /// Clones the options only for validation; it does not change the options
    /// passed to the provider. Returns the capability validation error when
    /// the request is unsupported.
    ///
    /// # Parameters
    /// - `options`: Read options to validate; explicit offset zero is treated
    ///   as an omitted offset for capability checking.
    /// - `capabilities`: Capabilities advertised by the configured provider.
    ///
    /// # Returns
    /// `Ok(())` when the requested operation is supported, or the capability
    /// validation error.
    pub(crate) fn validate_capabilities(options: &ReadOptions, capabilities: FileSystemCapabilities) -> FsResult<()> {
        let checked = if options.offset() == Some(0) {
            options.clone().with_offset(None)
        } else {
            options.clone()
        };
        checked.validate_against(capabilities)
    }
    /// Extracts the readable length from metadata returned with an opened
    /// random-access object.
    ///
    /// Missing metadata, a non-file kind, or a missing length is a
    /// `ProviderContractViolation`.
    ///
    /// # Parameters
    /// - `metadata`: Optional metadata returned with the opened object.
    ///
    /// # Returns
    /// The opened file length, or a provider-contract error if metadata does
    /// not identify a file with a length.
    pub(crate) fn opened_len(metadata: Option<&FileMetadata>) -> FsResult<u64> {
        let metadata = metadata.ok_or_else(|| contract("provider omitted metadata for opened random-access object"))?;
        if metadata.kind() != &FileKind::File {
            return Err(contract("provider opened a non-file random-access object"));
        }
        metadata
            .len()
            .ok_or_else(|| contract("provider omitted opened random-access object length"))
    }
    /// Checks a read range and returns its length clipped to the open-time
    /// bound.
    ///
    /// The order is deliberate: reject unrepresentable buffer lengths and
    /// offset-range overflow first; then return zero for an empty request or a
    /// request at or beyond the bound; finally enforce the provider's limit on
    /// the clipped range. Overflow and unrepresentable lengths return
    /// `InvalidOptions`; exceeding the range limit returns
    /// `ResourceLimitExceeded`. A zero result from the first two boundary
    /// cases requires no provider I/O and is not an EOF observation.
    ///
    /// # Parameters
    /// - `offset`: Requested starting byte offset.
    /// - `output_len`: Caller buffer length before clipping.
    /// - `opened_len`: Readable upper bound captured when the object opened.
    /// - `limit`: Maximum byte count permitted for one clipped provider read.
    ///
    /// # Returns
    /// The request length clipped to the open-time bound, or zero for an empty
    /// request or one starting at or beyond that bound.
    ///
    /// # Errors
    /// Returns an invalid-options error for unrepresentable lengths or range
    /// overflow, and a resource-limit error when the clipped request exceeds
    /// `limit`.
    pub(crate) fn validate_read(
        offset: u64,
        output_len: usize,
        opened_len: u64,
        limit: FileSystemLimit,
    ) -> FsResult<usize> {
        let requested = u64::try_from(output_len).map_err(|_| invalid("buffer length cannot be represented"))?;
        offset
            .checked_add(requested)
            .ok_or_else(|| invalid("read range overflows"))?;
        if requested == 0 || offset >= opened_len {
            return Ok(0);
        }
        let count = usize::try_from((opened_len - offset).min(requested))
            .map_err(|_| invalid("read range cannot be represented"))?;
        if limit.is_exceeded_by(count as u64) {
            return Err(FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::Read,
                "random-access read exceeds provider range limit",
            ));
        }
        Ok(count)
    }
}
/// Creates an invalid-options read error for an unrepresentable or overflowing
/// range.
///
/// # Parameters
/// - `s`: Static explanation of the invalid range.
///
/// # Returns
/// An invalid-options error for a read operation.
fn invalid(s: &str) -> FsError {
    FsError::new(FsErrorKind::InvalidOptions, FsOperation::Read, s)
}
/// Creates a provider-contract error for invalid opened-object metadata.
///
/// # Parameters
/// - `s`: Static explanation of the metadata contract violation.
///
/// # Returns
/// A provider-contract error for validating the provider outcome.
fn contract(s: &str) -> FsError {
    FsError::new(
        FsErrorKind::ProviderContractViolation,
        FsOperation::ValidateProviderOutcome,
        s,
    )
}
