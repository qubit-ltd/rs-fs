// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-neutral operation effect state.

/// Strongest known effect that a failed filesystem operation had on storage.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
#[must_use]
pub enum FsEffectState {
    /// The provider proved that the requested mutation did not occur.
    Unchanged,
    /// Some requested changes occurred, but the operation did not complete.
    PartiallyApplied,
    /// The requested namespace or data change occurred before a later failure.
    Applied,
    /// The provider cannot determine whether the requested change occurred.
    Indeterminate,
}
