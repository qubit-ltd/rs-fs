// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Stable configured filesystem limits.

use crate::FileSystemLimit;

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
    pub const fn with_max_path_text_bytes(mut self, limit: FileSystemLimit) -> Self {
        self.max_path_text_bytes = limit;
        self
    }

    /// Returns a copy with the component-text byte limit replaced by `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_component_text_bytes(mut self, limit: FileSystemLimit) -> Self {
        self.max_component_text_bytes = limit;
        self
    }

    /// Returns a copy with the range-read byte limit replaced by `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_read_range_bytes(mut self, limit: FileSystemLimit) -> Self {
        self.max_read_range_bytes = limit;
        self
    }

    /// Returns a copy with the write-session byte limit replaced by `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_write_bytes(mut self, limit: FileSystemLimit) -> Self {
        self.max_write_bytes = limit;
        self
    }

    /// Returns a copy with the native list-page entry limit replaced by `limit`.
    #[inline]
    #[must_use]
    pub const fn with_max_list_page_entries(mut self, limit: FileSystemLimit) -> Self {
        self.max_list_page_entries = limit;
        self
    }

    /// Returns the maximum canonical [`crate::FsPath`] text length in UTF-8 bytes.
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
}
