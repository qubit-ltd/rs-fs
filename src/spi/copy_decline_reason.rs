// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider copy-decline reasons.

/// Reason a provider declined its optional copy fast path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CopyDeclineReason {
    /// The provider does not implement a copy primitive.
    NotImplemented,
    /// The provider primitive cannot safely serve this particular request.
    NotApplicable,
}
