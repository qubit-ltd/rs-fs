// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::collections::BTreeSet;

/// Per-entry counters and duplicate-path state used by tree validation.
pub struct TreeEntryValidationState<'a> {
    /// Number of entries already accepted.
    pub entries_seen: usize,
    /// Measured content bytes already accepted.
    pub content_bytes_seen: u64,
    /// Actual bytes read for this entry so far.
    pub measured_content_bytes: u64,
    /// Canonical paths already accepted, updated only after validation.
    pub normalized_paths: &'a mut BTreeSet<String>,
}
