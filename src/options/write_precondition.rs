// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Preconditions for opening and committing writes.

use crate::ResourceVersion;

/// Version precondition applied to a write operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum WritePrecondition {
    /// Do not require a version precondition.
    #[default]
    None,
    /// Require that no destination exists.
    IfAbsent,
    /// Require the destination to have the supplied version.
    IfMatch(ResourceVersion),
}
