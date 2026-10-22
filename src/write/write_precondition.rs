// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Preconditions for opening and committing writes.

use crate::metadata::ResourceVersion;

/// Version precondition applied to a write operation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::ResourceVersion;
/// use qubit_fs::write::WritePrecondition;
///
/// let precondition = WritePrecondition::IfMatch(ResourceVersion::new("v1"));
/// let WritePrecondition::IfMatch(version) = precondition else {
///     panic!("expected a version match precondition");
/// };
/// assert_eq!(version.as_str(), "v1");
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum WritePrecondition {
    /// Do not require a version precondition.
    #[default]
    None,
    /// Require that no destination exists.
    IfAbsent,
    /// Require the destination to have the supplied version.
    IfMatch(
        /// Resource version that must match the current destination.
        ResourceVersion,
    ),
}
