// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared validation for provider-reported rename outcomes.

use super::rename_outcome_validation_error::RenameOutcomeValidationError;
use crate::metadata::AchievedAtomicity;
use crate::metadata::AtomicityRequirement;
use crate::metadata::DurabilityRequirement;
use crate::metadata::PublicationMethod;
use crate::path::Path;
use crate::rename::RenameFailureState;
use crate::rename::RenameOptions;
use crate::rename::RenameOutcome;

/// Validates provider-reported success against the requested rename guarantees.
///
/// The outcome must satisfy required atomicity and durability, must not use
/// copy-then-delete publication, and must identify the requested source and
/// target. Violations after reported success are classified with the
/// destination state implied by the reported outcome.
///
/// # Parameters
/// - `outcome`: Successful outcome reported by the provider.
/// - `options`: Guarantees requested for this rename.
/// - `source`: Source identity requested by the caller.
/// - `target`: Target identity requested by the caller.
///
/// # Returns
/// `Some` with a validation error when the provider outcome violates a
/// guarantee or reports different identities; otherwise, `None`.
pub(crate) fn validate_rename_outcome(
    outcome: &RenameOutcome,
    options: &RenameOptions,
    source: &Path,
    target: &Path,
) -> Option<RenameOutcomeValidationError> {
    if options.atomicity() == AtomicityRequirement::Required && outcome.atomicity() != AchievedAtomicity::Atomic {
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
    if options.durability() == DurabilityRequirement::Required && !outcome.durable() {
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
