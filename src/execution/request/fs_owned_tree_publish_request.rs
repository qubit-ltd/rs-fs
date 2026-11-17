// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use crate::metadata::ResourceVersion;
use crate::metadata::TreePublishOptions;
use crate::metadata::TreeValidationOptions;
use crate::path::Path;

/// Owned parameters for publishing a previously staged directory tree.
///
/// The provider validates and publishes the staging root as one request. The
/// validated version token can support an atomic compare-and-publish check;
/// it does not guarantee that a separate preflight remains valid.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::execution::FsOwnedTreePublishRequest;
/// use qubit_fs::metadata::TreePublishOptions;
/// use qubit_fs::metadata::TreeValidationOptions;
/// use qubit_fs::Path;
///
/// let request = FsOwnedTreePublishRequest {
///     staging_root: Path::parse("/staging/build-7").expect("static path is valid"),
///     destination_root: Path::parse("/releases/current").expect("static path is valid"),
///     validation: TreeValidationOptions::default(),
///     validated_version: None,
///     publication: TreePublishOptions::default(),
/// };
/// assert!(request.validated_version.is_none());
/// ```
pub struct FsOwnedTreePublishRequest {
    /// Validated staging root whose contents the provider validates and
    /// publishes.
    pub staging_root: Path,
    /// Validated destination root replaced or created by publication.
    pub destination_root: Path,
    /// Entry, depth, and aggregate-content limits applied before publication.
    pub validation: TreeValidationOptions,
    /// Version of the same staged owner observed after validation.
    ///
    /// Required atomic publication must compare this token as part of the
    /// publication transaction. It is not a standalone preflight guarantee.
    pub validated_version: Option<ResourceVersion>,
    /// Atomicity and durability guarantees required for publication.
    pub publication: TreePublishOptions,
}
