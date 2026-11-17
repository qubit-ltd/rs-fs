// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Outcome metadata returned by reads with an explicit consistency policy.

use crate::metadata::ResourceVersion;

/// Read data together with consistency observations from the opened object.
///
/// The version and change observation are optional because providers may not
/// expose those facts for a read session; their absence does not imply that the
/// content was stable.
///
/// # Type Parameters
///
/// - `T`: Value produced by the read operation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::ConsistentReadOutcome;
///
/// let outcome = ConsistentReadOutcome {
///     value: vec![1, 2, 3],
///     version: None,
///     changed: Some(false),
/// };
/// assert_eq!(vec![1, 2, 3], outcome.value);
/// assert_eq!(Some(false), outcome.changed);
/// assert!(outcome.version.is_none());
/// ```
#[must_use = "inspect the read value and its consistency observations"]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConsistentReadOutcome<T> {
    /// Complete or partial value produced by the read operation.
    pub value: T,
    /// Version token observed on the opened object, if the provider supplied
    /// one for this read session.
    pub version: Option<ResourceVersion>,
    /// Whether the provider observed a change during the read, or `None` when
    /// the session could not make that observation.
    pub changed: Option<bool>,
}
