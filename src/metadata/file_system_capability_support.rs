// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Support status for one filesystem capability.

/// Describes how a filesystem provider can satisfy a capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub enum FileSystemCapabilitySupport {
    /// The provider does not implement the capability.
    Unsupported,
    /// The provider can attempt the capability, but support depends on the
    /// request, authority, mount, or runtime state.
    Conditional,
    /// The provider supports the capability for every valid request in its
    /// advertised filesystem scope.
    Guaranteed,
}
