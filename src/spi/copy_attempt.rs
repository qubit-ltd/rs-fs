// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Optional provider copy results.

use super::CopyDeclineReason;
use crate::CopyOutcome;

/// Optional provider copy result.
#[non_exhaustive]
pub enum CopyAttempt {
    /// A provider completed the copy.
    Completed(CopyOutcome),
    /// The facade may select its later fallback.
    Declined(CopyDeclineReason),
}
