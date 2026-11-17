// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Privacy requirements for temporary filesystem resources.

/// Privacy strength requested for a temporary filesystem resource.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::metadata::PrivacyRequirement;
///
/// let requirement = PrivacyRequirement::Private;
/// assert_ne!(PrivacyRequirement::Preferred, requirement);
/// ```
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[must_use = "apply the privacy requirement to a temporary resource"]
pub enum PrivacyRequirement {
    /// Use provider defaults.
    #[default]
    Preferred,
    /// Restrict access to the creating principal.
    Private,
}
