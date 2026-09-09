// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- public prefix_read_policy_tests cover
// synchronous and asynchronous dispatch through this private policy.
//! Conservative provider request planning for bounded prefix reads.

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemProperties;
use crate::path::Path;
use crate::read::ChecksumPolicy;
use crate::read::ReadOptions;

/// Validated read options with an optional guaranteed provider-side bound.
pub(crate) struct PrefixReadPlan {
    /// Original options or a safely narrowed byte range.
    options: ReadOptions,
}

impl PrefixReadPlan {
    /// Validates the original request before considering a narrower range.
    ///
    /// # Errors
    /// Returns the original request validation error, or an unmet requirement
    /// when a partial read cannot establish complete checksum validation.
    pub(crate) fn new(
        properties: &FileSystemProperties,
        path: &Path,
        mut options: ReadOptions,
        max_bytes: usize,
    ) -> FsResult<Self> {
        let contextualize = |error: FsError| {
            error
                .with_operation(FsOperation::OpenReader)
                .with_path(path.clone())
                .with_missing_provider(properties.info().provider_id())
        };
        properties.validate_path(path, FsOperation::OpenReader)?;
        options
            .validate_against(properties.capabilities())
            .map_err(contextualize)?;
        properties
            .limits()
            .validate_read_range(path, options.length())
            .map_err(contextualize)?;
        if !properties.capabilities().supports(FileSystemCapability::Read) {
            return Err(contextualize(
                FsError::new(
                    FsErrorKind::UnsupportedCapability,
                    FsOperation::OpenReader,
                    "filesystem capability is not supported",
                )
                .with_required_capability(FileSystemCapability::Read),
            ));
        }
        if options.checksum() == ChecksumPolicy::Required {
            return Err(FsError::new(
                FsErrorKind::RequirementNotMet,
                FsOperation::Read,
                "prefix reads cannot confirm complete checksum validation",
            )
            .with_path(path.clone())
            .with_provider(properties.info().provider_id()));
        }
        if max_bytes > 0
            && options.checksum() == ChecksumPolicy::None
            && properties.capabilities().guarantees(FileSystemCapability::RangeRead)
            && let Ok(maximum) = u64::try_from(max_bytes)
        {
            let length = options.length().map_or(maximum, |value| value.min(maximum));
            let representable = options.offset().unwrap_or(0).checked_add(length).is_some();
            let allowed = !properties.limits().max_read_range_bytes().is_exceeded_by(length);
            if representable && allowed {
                options = options.with_length(Some(length));
            }
        }
        Ok(Self { options })
    }

    /// Transfers the validated provider request options to the reader open.
    #[inline]
    pub(crate) fn into_options(self) -> ReadOptions {
        self.options
    }
}
