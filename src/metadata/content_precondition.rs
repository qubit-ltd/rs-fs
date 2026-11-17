// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
/// Content-based precondition for a mutation.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::ContentPrecondition;
///
/// let precondition = ContentPrecondition::Equals(vec![1, 2, 3]);
/// assert_eq!(precondition, ContentPrecondition::Equals(vec![1, 2, 3]));
/// ```
#[must_use = "apply the content precondition to a mutation"]
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum ContentPrecondition {
    /// Do not compare existing content.
    #[default]
    None,
    /// Require current content to equal the supplied bytes.
    Equals(
        /// Expected content compared before the mutation is published.
        Vec<u8>,
    ),
}
