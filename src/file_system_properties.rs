// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.

//! Immutable filesystem property snapshots used by facades.

use crate::{
    FileSystemCapabilities,
    FileSystemInfo,
    FileSystemLimits,
    FsError,
    FsErrorKind,
    FsOperation,
    FsResult,
    PathConstraints,
    PathForm,
    PathSemantics,
    SymlinkPolicy,
};

/// Immutable construction-time properties cached by a filesystem facade.
#[derive(Clone, Debug)]
pub struct FileSystemProperties {
    /// Stable filesystem information.
    info: FileSystemInfo,
    /// Stable advertised capabilities.
    capabilities: FileSystemCapabilities,
    /// Stable provider limits.
    limits: FileSystemLimits,
    /// Accepted logical path forms.
    path_constraints: PathConstraints,
    /// Provider-declared symbolic-link traversal policy.
    symlink_policy: SymlinkPolicy,
}

impl FileSystemProperties {
    /// Builds and validates an immutable filesystem property snapshot.
    ///
    /// This method performs no I/O.
    ///
    /// # Parameters
    /// - `info`: Stable provider identity and path semantics.
    /// - `capabilities`: Capabilities explicitly advertised by the provider.
    /// - `limits`: Provider resource and operation limits.
    /// - `path_constraints`: Accepted absolute and relative path forms.
    /// - `symlink_policy`: Provider-declared symbolic-link traversal policy.
    ///
    /// # Returns
    /// A validated immutable property snapshot.
    ///
    /// # Errors
    /// Returns an invalid-options error when the provider identity is invalid,
    /// advertised capabilities violate dependencies, or path configuration is
    /// internally inconsistent.
    #[inline]
    pub fn new(
        info: FileSystemInfo,
        capabilities: FileSystemCapabilities,
        limits: FileSystemLimits,
        path_constraints: PathConstraints,
        symlink_policy: SymlinkPolicy,
    ) -> FsResult<Self> {
        let properties = Self {
            info,
            capabilities,
            limits,
            path_constraints,
            symlink_policy,
        };
        properties.validate()?;
        Ok(properties)
    }

    /// Returns the stable filesystem identity and configuration.
    ///
    /// # Returns
    /// The immutable provider information snapshot.
    #[inline(always)]
    #[must_use]
    pub const fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    /// Returns the stable advertised capabilities.
    ///
    /// # Returns
    /// Capabilities explicitly advertised by the provider.
    #[inline(always)]
    #[must_use]
    pub const fn capabilities(&self) -> FileSystemCapabilities {
        self.capabilities
    }

    /// Returns the stable filesystem limits.
    ///
    /// # Returns
    /// The immutable provider limit snapshot.
    #[inline(always)]
    #[must_use]
    pub const fn limits(&self) -> &FileSystemLimits {
        &self.limits
    }

    /// Returns the immutable accepted path constraints.
    ///
    /// # Returns
    /// The accepted logical path forms.
    #[inline(always)]
    #[must_use]
    pub const fn path_constraints(&self) -> &PathConstraints {
        &self.path_constraints
    }

    /// Returns the provider-declared symbolic-link traversal policy.
    #[inline(always)]
    #[must_use = "the filesystem symbolic-link policy must be used"]
    pub const fn symlink_policy(&self) -> SymlinkPolicy {
        self.symlink_policy
    }

    /// Defensively validates a provider-supplied snapshot at the facade
    /// boundary.
    ///
    /// It performs no I/O and is intentionally crate-private.
    ///
    /// # Returns
    /// `Ok(())` when all property invariants hold.
    ///
    /// # Errors
    /// Returns an invalid-options error when the snapshot violates core value
    /// invariants.
    pub(crate) fn validate(&self) -> FsResult<()> {
        if self.info.provider_id().is_empty()
            || self.info.provider_id().chars().any(char::is_control)
        {
            return Err(invalid_properties(
                "provider id must be non-empty and contain no controls",
            ));
        }
        if let Some((_capability, _dependency)) =
            self.capabilities.missing_dependency()
        {
            return Err(invalid_properties(
                "advertised capability dependency is missing",
            ));
        }
        if [
            self.limits.max_path_text_bytes(),
            self.limits.max_component_text_bytes(),
            self.limits.max_read_range_bytes(),
            self.limits.max_write_bytes(),
            self.limits.max_list_page_entries(),
        ]
        .into_iter()
        .any(|limit| matches!(limit, crate::FileSystemLimit::Maximum(0)))
        {
            return Err(invalid_properties(
                "finite filesystem limits must have a positive value",
            ));
        }
        if self.info.path_semantics() != PathSemantics::Hierarchical
            && self.path_constraints.form() == PathForm::Absolute
        {
            return Err(invalid_properties(
                "literal path semantics cannot require hierarchical absolute paths",
            ));
        }
        Ok(())
    }
}

/// Builds the shared property-validation failure.
///
/// # Parameters
/// - `message`: Static explanation of the violated property invariant.
///
/// # Returns
/// An invalid-options error scoped to provider configuration.
fn invalid_properties(message: &'static str) -> FsError {
    FsError::new(
        FsErrorKind::InvalidOptions,
        FsOperation::ValidateProperties,
        message,
    )
}
