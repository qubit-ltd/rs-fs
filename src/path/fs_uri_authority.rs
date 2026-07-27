// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Explicit filesystem URI authority-component state.

use crate::FsAuthority;

/// Explicit authority-component state for a filesystem URI.
///
/// URI syntax distinguishes an absent authority from an explicitly empty
/// authority. This type preserves that distinction when constructing URIs from
/// validated components.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FsUriAuthority {
    /// The URI has no `//` authority component.
    Absent,
    /// The URI has an explicitly empty `//` authority component.
    Empty,
    /// The URI has a non-empty validated authority component.
    Present(FsAuthority),
}
