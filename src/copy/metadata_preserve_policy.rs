// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Metadata preservation policy.

/// Metadata categories requested for a copy operation.
///
/// These variants select categories, not a cross-provider list of fields.
/// Each provider must document the fields it supports for each operation,
/// including required, best-effort, and excluded fields and any platform or
/// resource-kind differences. In particular, `Portable` does not guarantee
/// that all [`FileMetadata`](crate::metadata::FileMetadata) fields are copied.
/// Applications that require specific fields must consult the provider's
/// documentation for the operation and verify the destination as needed.
///
/// A provider that cannot support a requested category must report
/// [`RequirementNotMet`](crate::error::FsErrorKind::RequirementNotMet) when
/// that can be determined before I/O. A failure while setting fields must
/// preserve the actual error and any publication facts; it must not be
/// reported as unchanged or silently retried.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::copy::MetadataPreservePolicy;
///
/// assert_eq!(MetadataPreservePolicy::Portable, MetadataPreservePolicy::default());
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MetadataPreservePolicy {
    /// Do not explicitly copy source metadata.
    ///
    /// Native copy or rename behavior may still give the destination intrinsic
    /// attributes; this policy does not require the destination to be empty or
    /// different from the source.
    None,
    /// Request the provider's documented set of writable, general-purpose
    /// attributes for this operation.
    ///
    /// There is no uniform non-empty field set across providers. This variant
    /// does not imply preservation of timestamps, content type, or every
    /// [`FileMetadata`](crate::metadata::FileMetadata) field.
    Portable,
    /// Request the user-defined metadata category supported by the provider.
    ///
    /// This category does not implicitly include [`Portable`](Self::Portable)
    /// or provider-native attributes.
    UserMetadata,
    /// Request provider-native attributes representable for this operation.
    ///
    /// The provider documents its supported and best-effort behavior. This
    /// does not promise lossless conversion of native extensions between
    /// providers.
    ProviderNative,
    /// Request the union of the above categories that this operation can
    /// represent.
    ///
    /// Unrepresentable fields must be documented as excluded. This does not
    /// copy resource identity, generated version identifiers, or every
    /// read-only observed field such as length or checksum.
    All,
}

impl Default for MetadataPreservePolicy {
    /// Preserves portable metadata by default.
    #[inline]
    fn default() -> Self {
        Self::Portable
    }
}
