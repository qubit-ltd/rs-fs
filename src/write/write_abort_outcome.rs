// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Destination publication certainty after writer cleanup.

/// Provider-confirmed destination publication state after writer cleanup
/// completes. Every successful outcome confirms cleanup; `Indeterminate`
/// means only that the destination publication state is unknown. A provider
/// that cannot confirm cleanup must return an error instead.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::write::WriteAbortOutcome;
///
/// let outcome = WriteAbortOutcome::Published;
/// let destination_may_have_changed = match outcome {
///     WriteAbortOutcome::NotPublished => false,
///     WriteAbortOutcome::Published | WriteAbortOutcome::Indeterminate => true,
/// };
/// assert!(destination_may_have_changed);
/// ```
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[must_use]
pub enum WriteAbortOutcome {
    /// Cleanup completed and the destination was proven unchanged.
    NotPublished,
    /// Cleanup completed after the destination had already changed.
    Published,
    /// Cleanup completed, but the destination state remains unknown.
    Indeterminate,
}
