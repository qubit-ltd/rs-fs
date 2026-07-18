// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stable configured filesystem limits.

/// Optional limits known when a configured filesystem is constructed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FileSystemLimits {
    /// Maximum accepted provider-local path length in UTF-8 bytes.
    pub max_path_bytes: Option<usize>,
    /// Maximum accepted single path-component length in UTF-8 bytes.
    pub max_component_bytes: Option<usize>,
    /// Maximum byte count accepted by one required range read.
    pub max_read_range_bytes: Option<u64>,
    /// Maximum byte count accepted by one write session when known.
    pub max_write_bytes: Option<u64>,
    /// Maximum provider-native directory page size when known.
    pub max_list_page_size: Option<usize>,
}
