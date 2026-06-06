// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Delete operation options.

/// Options controlling delete operations.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DeleteOptions {
    /// Whether container resources should be removed recursively.
    pub recursive: bool,
    /// Whether a missing target should be treated as success.
    pub missing_ok: bool,
    /// Optional required ETag or provider version.
    pub if_match: Option<String>,
}
