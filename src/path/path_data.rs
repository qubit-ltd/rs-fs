// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::sync::Arc;

use super::PathSemantics;

/// Immutable internal representation shared by cloned logical paths.
#[derive(Debug, Eq, Hash, PartialEq)]
pub(super) struct PathData {
    /// Whether this logical path starts at a provider root.
    pub(super) absolute: bool,
    /// Canonical normalized or provider-literal text.
    pub(super) text: Arc<str>,
    /// Whether component iteration must preserve literal slash boundaries.
    pub(super) literal: bool,
    /// Semantics used to validate this spelling.
    pub(super) semantics: PathSemantics,
}
