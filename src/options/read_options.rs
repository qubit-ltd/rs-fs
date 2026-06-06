// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Read operation options.

use crate::ChecksumPolicy;

/// Options controlling a read operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ReadOptions {
    /// Optional byte offset.
    pub offset: Option<u64>,
    /// Optional byte length.
    pub length: Option<u64>,
    /// Optional required ETag or provider version.
    pub if_match: Option<String>,
    /// Optional ETag or provider version that must not match.
    pub if_none_match: Option<String>,
    /// Checksum validation policy.
    pub checksum: ChecksumPolicy,
}
