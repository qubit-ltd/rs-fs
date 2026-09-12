// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================

//! Static copy execution assessment.

use super::CopyExecutionRoute;
use super::FallbackRejection;

/// Result of no-I/O copy route analysis.
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
