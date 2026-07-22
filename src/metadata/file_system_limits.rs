// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Stable configured filesystem limits.

use crate::{
    FileSystemLimit,
    FsError,
    FsErrorKind,
    FsOperation,
    FsPath,
    FsResult,
    PathSemantics,
};

/// Stable limits declared by a configured filesystem provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileSystemLimits {
    max_path_text_bytes: FileSystemLimit,
    max_component_text_bytes: FileSystemLimit,
    max_read_range_bytes: FileSystemLimit,
    max_write_bytes: FileSystemLimit,
    max_list_page_entries: FileSystemLimit,
}

impl FileSystemLimits {
    /// Creates a limit snapshot whose dimensions are all explicitly unknown.
    #[inline]
    #[must_use]
    pub const fn unknown() -> Self {
        Self {
            max_path_text_bytes: FileSystemLimit::Unknown,
            max_component_text_bytes: FileSystemLimit::Unknown,
            max_read_range_bytes: FileSystemLimit::Unknown,
            max_write_bytes: FileSystemLimit::Unknown,
            max_list_page_entries: FileSystemLimit::Unknown,
        }
    }

    /// Returns a copy with the path-text byte limit replaced by `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_path_text_bytes(
        mut self,
        limit: FileSystemLimit,
    ) -> Self {
        self.max_path_text_bytes = limit;
        self
    }

    /// Returns a copy with the component-text byte limit replaced by `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_component_text_bytes(
        mut self,
        limit: FileSystemLimit,
    ) -> Self {
        self.max_component_text_bytes = limit;
        self
    }

    /// Returns a copy with the range-read byte limit replaced by `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_read_range_bytes(
        mut self,
        limit: FileSystemLimit,
    ) -> Self {
        self.max_read_range_bytes = limit;
        self
    }

    /// Returns a copy with the write-session byte limit replaced by `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_write_bytes(
        mut self,
        limit: FileSystemLimit,
    ) -> Self {
        self.max_write_bytes = limit;
        self
    }

    /// Returns a copy with the native list-page entry limit replaced by
    /// `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_list_page_entries(
        mut self,
        limit: FileSystemLimit,
    ) -> Self {
        self.max_list_page_entries = limit;
        self
    }

    /// Returns the maximum canonical [`crate::FsPath`] text length in UTF-8
    /// bytes.
    #[inline(always)]
    #[must_use]
    pub const fn max_path_text_bytes(&self) -> FileSystemLimit {
        self.max_path_text_bytes
    }

    /// Returns the maximum path-component text length in UTF-8 bytes.
    #[inline(always)]
    #[must_use]
    pub const fn max_component_text_bytes(&self) -> FileSystemLimit {
        self.max_component_text_bytes
    }

    /// Returns the maximum byte count accepted by one logical range read.
    #[inline(always)]
    #[must_use]
    pub const fn max_read_range_bytes(&self) -> FileSystemLimit {
        self.max_read_range_bytes
    }

    /// Returns the maximum total byte count accepted by one write session.
    #[inline(always)]
    #[must_use]
    pub const fn max_write_bytes(&self) -> FileSystemLimit {
        self.max_write_bytes
    }

    /// Returns the maximum entry count in one provider-native list page.
    #[inline(always)]
    #[must_use]
    pub const fn max_list_page_entries(&self) -> FileSystemLimit {
        self.max_list_page_entries
    }

    /// Clamps a requested list-page size to the declared finite maximum.
    ///
    /// Unknown, unbounded, and inapplicable limits leave the hint unchanged.
    /// A missing hint remains absent so the provider can select its natural
    /// page size while still honoring its declared limit.
    ///
    /// # Parameters
    /// - `requested`: Optional caller-supplied page-size hint.
    ///
    /// # Returns
    /// The effective page-size hint forwarded to the provider.
    #[inline]
    #[must_use]
    pub fn clamp_list_page_size(
        &self,
        requested: Option<usize>,
    ) -> Option<usize> {
        let requested = requested?;
        match self.max_list_page_entries {
            FileSystemLimit::Maximum(maximum) => usize::try_from(maximum)
                .map_or(Some(requested), |maximum| {
                    Some(requested.min(maximum))
                }),
            FileSystemLimit::Unknown
            | FileSystemLimit::NotApplicable
            | FileSystemLimit::Unbounded => Some(requested),
        }
    }

    /// Validates a canonical filesystem path against provider path limits.
    ///
    /// Component limits are checked only for hierarchical path semantics.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::ResourceLimitExceeded`] when the complete path
    /// text or a hierarchical component exceeds its declared finite maximum.
    pub fn validate_path(
        &self,
        path: &FsPath,
        semantics: PathSemantics,
        operation: FsOperation,
    ) -> FsResult<()> {
        if exceeds_usize(self.max_path_text_bytes, path.as_str().len()) {
            return Err(limit_error(
                operation,
                "path text exceeds the provider byte limit",
                path,
            ));
        }
        if semantics == PathSemantics::Hierarchical
            && path.as_str().split('/').any(|component| {
                !component.is_empty()
                    && exceeds_usize(
                        self.max_component_text_bytes,
                        component.len(),
                    )
            })
        {
            return Err(limit_error(
                operation,
                "path component exceeds the provider byte limit",
                path,
            ));
        }
        Ok(())
    }

    /// Validates a requested logical range-read length.
    ///
    /// A missing length cannot be preflighted and remains the provider's
    /// responsibility during execution.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::ResourceLimitExceeded`] when `length` exceeds
    /// the declared finite range-read maximum.
    pub fn validate_read_range(
        &self,
        path: &FsPath,
        length: Option<u64>,
    ) -> FsResult<()> {
        if length.is_some_and(|length| {
            self.max_read_range_bytes.is_exceeded_by(length)
        }) {
            Err(limit_error(
                FsOperation::OpenReader,
                "requested range exceeds the provider byte limit",
                path,
            ))
        } else {
            Ok(())
        }
    }

    /// Validates the total bytes supplied to one write session.
    ///
    /// # Errors
    /// Returns [`FsErrorKind::ResourceLimitExceeded`] when `bytes` exceeds the
    /// declared finite write-session maximum.
    pub fn validate_write_size(
        &self,
        path: &FsPath,
        bytes: usize,
    ) -> FsResult<()> {
        if exceeds_usize(self.max_write_bytes, bytes) {
            Err(limit_error(
                FsOperation::Write,
                "write session exceeds the provider byte limit",
                path,
            ))
        } else {
            Ok(())
        }
    }
}

fn exceeds_usize(limit: FileSystemLimit, actual: usize) -> bool {
    match u64::try_from(actual) {
        Ok(actual) => limit.is_exceeded_by(actual),
        Err(_) => matches!(limit, FileSystemLimit::Maximum(_)),
    }
}

fn limit_error(
    operation: FsOperation,
    message: &'static str,
    path: &FsPath,
) -> FsError {
    FsError::new(FsErrorKind::ResourceLimitExceeded, operation, message)
        .with_path(path.clone())
}
