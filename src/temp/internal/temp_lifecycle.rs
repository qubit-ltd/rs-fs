// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private temporary-resource recovery snapshot and transitions.
use crate::error::FsError;
use crate::path::Path;
use crate::temp::PersistFailureState;
use crate::temp::TempResourceState;

/// Tracks source ownership separately from target publication.
#[derive(Clone)]
pub(crate) struct TempLifecycle {
    /// Current handle ownership state.
    state: TempResourceState,
    /// Publication and source-release facts for later failures.
    failure_state: PersistFailureState,
    /// Last positively confirmed publication destination.
    publication_target: Option<Path>,
}
impl TempLifecycle {
    /// Creates an owned, unpublished resource lifecycle.
    #[inline]
    pub(crate) const fn new() -> Self {
        Self {
            state: TempResourceState::Owned,
            failure_state: PersistFailureState::NotPublished,
            publication_target: None,
        }
    }
    /// Returns the current handle ownership state.
    #[inline]
    pub(crate) const fn state(&self) -> TempResourceState {
        self.state
    }
    /// Returns retained publication and source-release facts.
    #[inline]
    pub(crate) const fn failure_state(&self) -> PersistFailureState {
        self.failure_state
    }
    /// Returns the confirmed destination, if publication has been observed.
    #[inline]
    pub(crate) fn publication_target(&self) -> Option<&Path> {
        self.publication_target.as_ref()
    }
    /// Marks an in-flight mutation uncertain before awaiting the provider.
    #[cfg(feature = "async")]
    #[inline]
    pub(crate) fn begin_pending(&mut self) {
        self.state = TempResourceState::Indeterminate;
        self.failure_state = PersistFailureState::Indeterminate;
    }
    /// Records confirmed publication and released source ownership.
    #[inline]
    pub(crate) fn record_success(&mut self, kept: bool, target: Path) {
        self.state = if kept {
            TempResourceState::Kept
        } else {
            TempResourceState::Persisted
        };
        self.failure_state = PersistFailureState::PublishedSourceReleased;
        self.publication_target = Some(target);
    }
    /// Records provider failure without discarding an earlier known target.
    #[inline]
    pub(crate) fn record_failure(&mut self, state: PersistFailureState, target: Option<Path>, kept: bool) {
        if matches!(
            state,
            PersistFailureState::PublishedSourceRetained
                | PersistFailureState::PublishedSourceReleased
                | PersistFailureState::PublishedSourceIndeterminate
        ) && let Some(target) = target
        {
            self.publication_target = Some(target);
        }
        self.failure_state = state;
        self.state = match state {
            PersistFailureState::NotPublished => TempResourceState::Owned,
            PersistFailureState::NotPublishedSourceReleased => TempResourceState::Cleaned,
            PersistFailureState::PublishedSourceRetained | PersistFailureState::NotPublishedSourceCleanupRequired => {
                TempResourceState::CleanupRequired
            }
            PersistFailureState::PublishedSourceReleased => {
                if kept {
                    TempResourceState::Kept
                } else {
                    TempResourceState::Persisted
                }
            }
            PersistFailureState::Indeterminate
            | PersistFailureState::NotPublishedSourceIndeterminate
            | PersistFailureState::PublishedSourceIndeterminate => TempResourceState::Indeterminate,
        };
    }
    /// Releases source ownership while preserving publication certainty.
    #[inline]
    pub(crate) fn record_cleanup_success(&mut self) {
        self.state = TempResourceState::Cleaned;
        if self.publication_target.is_some() {
            self.failure_state = PersistFailureState::PublishedSourceReleased;
            return;
        }
        self.failure_state = match self.failure_state {
            PersistFailureState::PublishedSourceRetained
            | PersistFailureState::PublishedSourceReleased
            | PersistFailureState::PublishedSourceIndeterminate => PersistFailureState::PublishedSourceReleased,
            PersistFailureState::Indeterminate => PersistFailureState::Indeterminate,
            _ => PersistFailureState::NotPublishedSourceReleased,
        };
    }
    /// Preserves uncertainty or records outstanding cleanup responsibility.
    #[inline]
    pub(crate) fn record_cleanup_error(&mut self, error: &FsError) {
        if self.state == TempResourceState::Indeterminate {
            return;
        }
        if error.has_indeterminate_effect() {
            self.state = TempResourceState::Indeterminate;
            self.failure_state = if self.publication_target.is_some()
                || matches!(
                    self.failure_state,
                    PersistFailureState::PublishedSourceRetained | PersistFailureState::PublishedSourceReleased
                ) {
                PersistFailureState::PublishedSourceIndeterminate
            } else {
                PersistFailureState::NotPublishedSourceIndeterminate
            };
        } else {
            self.state = TempResourceState::CleanupRequired;
            if self.failure_state == PersistFailureState::NotPublished {
                self.failure_state = PersistFailureState::NotPublishedSourceCleanupRequired;
            }
        }
    }
}
