// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared validation for provider-reported rename outcomes.

use super::rename_outcome_validation_error::RenameOutcomeValidationError;
use crate::{
    AchievedAtomicity,
    DurabilityRequirement,
    Path,
    PublicationMethod,
    RenameFailureState,
    RenameOptions,
    RenameOutcome,
};

/// Validates provider outcome guarantees shared by sync and async facades.
pub(crate) fn validate_rename_outcome(
    outcome: &RenameOutcome,
    options: &RenameOptions,
    source: &Path,
    target: &Path,
) -> Option<RenameOutcomeValidationError> {
    if options.atomicity() == crate::AtomicityRequirement::Required
        && outcome.atomicity() != AchievedAtomicity::Atomic
    {
        return Some(RenameOutcomeValidationError {
            message: "provider reported non-atomic success for an atomic-required rename",
            state: RenameFailureState::Renamed,
        });
    }
    if outcome.method() == PublicationMethod::CopyThenDelete {
        return Some(RenameOutcomeValidationError {
            message: "provider returned copy-then-delete for rename",
            state: RenameFailureState::Renamed,
        });
    }
    if options.durability() == DurabilityRequirement::Required
        && !outcome.durable()
    {
        return Some(RenameOutcomeValidationError {
            message: "provider reported non-durable success for a durability-required rename",
            state: RenameFailureState::Renamed,
        });
    }
    if outcome.source() != source || outcome.target() != target {
        return Some(RenameOutcomeValidationError {
            message: "provider returned a rename outcome with different identities",
            state: RenameFailureState::Indeterminate,
        });
    }
    None
}
