// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair
//! Methods used to publish filesystem changes.

/// Concrete method used to publish a successful filesystem change.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PublicationMethod {
    /// Bytes were written directly to the destination.
    Direct,
    /// A staging resource was atomically renamed into place.
    AtomicRename,
    /// A conditional provider operation replaced the destination.
    ConditionalReplace,
    /// A provider-native server-side copy published the destination.
    ServerSideCopy,
    /// Bytes were streamed through the caller into the destination.
    StreamCopy,
    /// A copy was followed by deletion of the source.
    CopyThenDelete,
}
