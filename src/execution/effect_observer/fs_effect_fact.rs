// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
use std::sync::Arc;

use qubit_operation::RecoveryId;

use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
use crate::directory::TreePublishOutcome;
use crate::error::FsEffectState;
use crate::error::FsError;
use crate::rename::RenameFailureState;
use crate::write::WriteFailureState;

/// Typed final evidence retained independently of the operation outcome.
///
/// # Examples
///
/// ```rust
/// use qubit_fs::FsEffectFact;
/// use qubit_fs::FsEffectState;
///
/// let fact = FsEffectFact::State(FsEffectState::Unchanged);
/// assert!(matches!(fact, FsEffectFact::State(FsEffectState::Unchanged)));
/// ```
#[must_use]
#[derive(Clone, Debug)]
pub enum FsEffectFact {
    /// Strongest confirmed storage effect.
    State(
        /// Provider-confirmed state of the requested mutation.
        FsEffectState,
    ),
    /// Shareable copy facts, with physical recovery ownership held elsewhere.
    CopyFailure {
        /// Original causal filesystem error.
        error: Arc<FsError>,
        /// Confirmed publication state.
        state: CopyFailureState,
        /// Confirmed transfer counters.
        partial_stats: CopyStats,
        /// Recovery record identifier, when one owns a retained resource.
        recovery: Option<RecoveryId>,
    },
    /// Shareable write facts, with physical recovery ownership held elsewhere.
    WriteFailure {
        /// Original causal filesystem error.
        error: Arc<FsError>,
        /// Confirmed publication state.
        state: WriteFailureState,
        /// Confirmed byte count.
        written_bytes: u64,
        /// Recovery record identifier, when one owns a retained resource.
        recovery: Option<RecoveryId>,
    },
    /// Shareable rename facts.
    RenameFailure {
        /// Original causal filesystem error.
        error: Arc<FsError>,
        /// Provider-confirmed namespace transition.
        state: RenameFailureState,
    },
    /// Provider-confirmed staged-tree publication facts.
    TreePublish {
        /// Actual publication outcome, including partial counts and guarantees.
        outcome: Arc<TreePublishOutcome>,
        /// Recovery record retaining a failed publication owner, when present.
        recovery: Option<RecoveryId>,
    },
}
