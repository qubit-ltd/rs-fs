// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================

//! Static copy execution assessment.

use super::CopyExecutionRoute;
use super::FallbackRejection;

/// Result of no-I/O copy route analysis.
///
/// The assessment is derived from the cached provider snapshot and does not
/// contact the provider. Execution may still fail after this assessment.
///
/// # Examples
///
/// ```rust
/// # fn main() -> Result<(), qubit_fs::FsError> {
/// use qubit_fs::Path;
/// use qubit_fs::copy::CopyExecutionRoute;
/// use qubit_fs::copy::CopyOptions;
/// let fs = qubit_fs::rustdoc_provider::filesystem();
/// let assessment = fs.assess_copy(&Path::parse("/report")?, &Path::parse("/copy")?, &CopyOptions::default())?;
/// assert_eq!(assessment.route(), CopyExecutionRoute::StreamOnly);
/// assert_eq!(assessment.fallback_rejection(), None);
/// # Ok(())
/// # }
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CopyAssessment {
    route: CopyExecutionRoute,
    fallback_rejection: Option<FallbackRejection>,
}

impl CopyAssessment {
    /// Creates an assessment inside the facade.
    #[inline]
    pub(crate) const fn new(route: CopyExecutionRoute, fallback_rejection: Option<FallbackRejection>) -> Self {
        Self {
            route,
            fallback_rejection,
        }
    }

    /// Returns the possible execution route.
    #[inline]
    #[must_use]
    pub const fn route(self) -> CopyExecutionRoute {
        self.route
    }

    /// Returns the first static fallback rejection, if any.
    #[inline]
    #[must_use]
    pub const fn fallback_rejection(self) -> Option<FallbackRejection> {
        self.fallback_rejection
    }
}
