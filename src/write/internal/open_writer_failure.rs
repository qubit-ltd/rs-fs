// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Stage-specific certainty for a failed writer open.

use crate::error::FsEffectState;
use crate::error::FsError;
use crate::write::WriteFailureState;

/// Reports whether an unsuccessful open proved no external effect or retained
/// cleanup responsibility. An indeterminate error kind overrides a
/// contradictory unchanged annotation. Applied opening effects do not prove
/// file publication.
#[inline]
#[must_use]
pub(crate) fn is_unchanged_open_failure(error: &FsError) -> bool {
    error.effect_state() == Some(FsEffectState::Unchanged) && !error.has_indeterminate_effect()
}

/// Maps opening evidence to whole-file publication certainty.
///
/// Missing evidence and effects of staging or parent creation are
/// conservatively indeterminate. This function must not classify commit or
/// cleanup failures.
#[inline]
#[must_use]
pub(crate) fn open_failure_state(error: &FsError) -> WriteFailureState {
    if is_unchanged_open_failure(error) {
        WriteFailureState::NotPublished
    } else {
        WriteFailureState::Indeterminate
    }
}
