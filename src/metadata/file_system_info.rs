// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Immutable configured filesystem information.

use std::fmt::Display;

use crate::error::FsResult;
use crate::metadata::FileSystemId;
use crate::metadata::NonSensitiveMetadata;
use crate::metadata::UserMetadata;
use crate::path::PathSemantics;
use crate::path::Uri;

/// Construction-time local snapshot describing one filesystem object.
///
/// # Examples
///
/// ```
/// use qubit_fs::metadata::FileSystemId;
/// use qubit_fs::metadata::FileSystemInfo;
/// use qubit_fs::path::PathSemantics;
///
/// let info = FileSystemInfo::new(
///     FileSystemId::new("local-instance")?,
///     "local",
///     PathSemantics::Hierarchical,
/// );
/// assert_eq!("local", info.provider_id());
/// # Ok::<(), qubit_fs::FsError>(())
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct FileSystemInfo {
    /// Stable identity of the configured filesystem.
    id: FileSystemId,
    /// Stable identity of the provider implementation.
    provider_id: Box<str>,
    /// Validated URI schemes accepted by this filesystem.
    schemes: Vec<String>,
    /// Logical path model used by the provider.
    path_semantics: PathSemantics,
    /// Provider metadata safe for automatic structural formatting.
    provider_metadata: NonSensitiveMetadata,
}

impl FileSystemInfo {
    /// Creates a filesystem information snapshot without scheme aliases.
    #[inline(always)]
    #[must_use]
    pub fn new(id: FileSystemId, provider_id: impl Display, path_semantics: PathSemantics) -> Self {
        Self {
            id,
            provider_id: provider_id.to_string().into(),
            schemes: Vec::new(),
            path_semantics,
            provider_metadata: NonSensitiveMetadata::new(),
        }
    }

    /// Adds one validated supported URI scheme.
    ///
    /// # Errors
    ///
    /// Returns an invalid-URI error when `scheme` is not a valid URI scheme.
    pub fn with_scheme(mut self, scheme: &str) -> FsResult<Self> {
        let scheme = Uri::parse(&format!("{scheme}:/"))?.scheme().to_owned();
        if !self.schemes.contains(&scheme) {
            self.schemes.push(scheme);
        }
        Ok(self)
    }

    /// Replaces the scrubbed provider metadata snapshot.
    ///
    /// `metadata` has already rejected credential-like keys. Providers must
    /// expose secrets only through an external credential boundary, never
    /// through this debug-visible local snapshot.
    #[inline(always)]
    #[must_use]
    pub fn with_provider_metadata(mut self, metadata: UserMetadata) -> Self {
        self.provider_metadata = NonSensitiveMetadata::from(metadata);
        self
    }

    /// Returns the configured filesystem identity.
    #[inline]
    #[must_use]
    pub const fn id(&self) -> &FileSystemId {
        &self.id
    }

    /// Returns the provider identity that created this filesystem.
    #[inline]
    #[must_use]
    pub const fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns supported URI schemes in provider-defined order.
    #[inline]
    #[must_use]
    pub fn schemes(&self) -> &[String] {
        &self.schemes
    }

    /// Returns the provider-local path semantics.
    #[inline]
    #[must_use]
    pub const fn path_semantics(&self) -> PathSemantics {
        self.path_semantics
    }

    /// Returns scrubbed provider-specific information.
    #[inline]
    #[must_use]
    pub const fn provider_metadata(&self) -> &NonSensitiveMetadata {
        &self.provider_metadata
    }
}

#[cfg(test)]
mod tests {
    use super::FileSystemInfo;
    use crate::metadata::FileSystemId;
    use crate::metadata::UserMetadata;
    use crate::path::PathSemantics;

    #[test]
    fn runtime_contract_covers_snapshot_accessors() {
        let id = FileSystemId::new("file-system-info-test").expect("valid id");
        let info = FileSystemInfo::new(id.clone(), "provider", PathSemantics::Hierarchical)
            .with_scheme("file")
            .expect("valid scheme")
            .with_provider_metadata(UserMetadata::new());

        assert_eq!(info.id(), &id);
        assert_eq!(info.provider_id(), "provider");
        assert_eq!(info.schemes(), &["file".to_owned()]);
        assert_eq!(info.path_semantics(), PathSemantics::Hierarchical);
        assert!(info.provider_metadata().is_empty());
    }
}
