// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Static copy execution assessment.

use super::CopyExecutionRoute;
use super::FallbackRejection;

/// Result of no-I/O copy route analysis.
///
/// The assessment is derived from the cached provider snapshot and does not
/// contact the provider. It checks paths, options and capabilities before
/// same-path validation, and reports a zero entry budget as
/// [`FsErrorKind::ResourceLimitExceeded`](crate::error::FsErrorKind::ResourceLimitExceeded). It does not check the
/// dynamic execution deadline. Execution may still fail after this assessment.
///
/// # Examples
///
/// The application supplies a configured filesystem and validated paths.
///
/// ```rust
/// use qubit_fs::FileSystem;
/// use qubit_fs::Path;
/// use qubit_fs::copy::{CopyExecutionRoute, CopyOptions};
///
/// fn inspect(filesystem: &FileSystem, source: &Path, target: &Path) -> qubit_fs::FsResult<()> {
///     let assessment = filesystem.assess_copy(source, target, &CopyOptions::default())?;
///     match assessment.route() {
///         CopyExecutionRoute::ProviderThenStream => { /* provider attempt, then fallback */ }
///         CopyExecutionRoute::ProviderOnly => { /* provider primitive is required */ }
///         CopyExecutionRoute::StreamOnly => { /* use the stream route */ }
///         _ => {}
///     }
///     if let Some(reason) = assessment.fallback_rejection() {
///         println!("stream fallback is unavailable: {reason:?}");
///     }
///     Ok(())
/// }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[must_use]
pub struct CopyAssessment {
    /// Route selected for attempting the requested copy.
    route: CopyExecutionRoute,
    /// Static reason the stream fallback cannot meet the request, if any.
    fallback_rejection: Option<FallbackRejection>,
}

impl CopyAssessment {
    /// Creates an assessment inside the facade.
    ///
    /// # Parameters
    ///
    /// - `route`: Copy execution route selected from provider and fallback
    ///   support.
    /// - `fallback_rejection`: Static reason the stream fallback cannot meet
    ///   the request, if any.
    ///
    /// # Returns
    /// An assessment retaining the selected route and fallback constraint.
    #[inline]
    pub(crate) const fn new(route: CopyExecutionRoute, fallback_rejection: Option<FallbackRejection>) -> Self {
        Self {
            route,
            fallback_rejection,
        }
    }

    /// Returns the possible execution route.
    ///
    /// # Returns
    /// The route selected for the requested copy.
    #[inline]
    pub const fn route(self) -> CopyExecutionRoute {
        self.route
    }

    /// Returns the first static fallback rejection, if any.
    ///
    /// # Returns
    /// `Some` with the rejection reason when fallback is unavailable, or `None`
    /// otherwise.
    #[inline]
    pub const fn fallback_rejection(self) -> Option<FallbackRejection> {
        self.fallback_rejection
    }
}
