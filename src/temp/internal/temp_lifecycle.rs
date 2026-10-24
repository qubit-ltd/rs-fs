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

/// Tracks source cleanup ownership independently from target publication.
///
/// The snapshot retains confirmed publication data across later cleanup
/// failures so callers can report partial progress accurately.
#[derive(Clone)]
pub(crate) struct TempLifecycle {
    /// Cleanup responsibility currently held by the temporary handle.
    state: TempResourceState,
    /// Provider-confirmed publication and source-release facts used to
    /// describe later failures.
    failure_state: PersistFailureState,
    /// Most recent destination whose publication was positively confirmed.
    publication_target: Option<Path>,
}
impl TempLifecycle {
    /// Creates a lifecycle that owns cleanup for an unpublished resource.
    #[inline]
    #[must_use]
    pub(crate) const fn new() -> Self {
        Self {
            state: TempResourceState::Owned,
            failure_state: PersistFailureState::NotPublished,
            publication_target: None,
        }
    }
    /// Returns whether this handle owns cleanup, has released it, or cannot
    /// determine the resource state.
    #[inline]
    pub(crate) const fn state(&self) -> TempResourceState {
        self.state
    }
    /// Returns provider-confirmed publication and source-release progress for
    /// interpreting persistence failures.
    #[must_use]
    #[inline]
    pub(crate) const fn failure_state(&self) -> PersistFailureState {
        self.failure_state
    }
    /// Returns the last confirmed publication destination, if publication has
    /// been observed.
    ///
    /// The borrowed path remains tied to this lifecycle snapshot.
    #[must_use]
    #[inline]
    pub(crate) fn publication_target(&self) -> Option<&Path> {
        self.publication_target.as_ref()
    }
    /// Marks an in-flight provider mutation uncertain before it is awaited.
    ///
    /// This prevents cleanup from treating an unconfirmed source state as
    /// safely owned.
    #[cfg(feature = "async")]
    #[inline]
    pub(crate) fn begin_pending(&mut self) {
        self.state = TempResourceState::Indeterminate;
        self.failure_state = PersistFailureState::Indeterminate;
    }
    /// Records successful publication and release of source cleanup ownership.
    ///
    /// `kept` distinguishes a retained resource from one whose source was
    /// persisted, while `target` records the confirmed destination.
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
    /// Records provider-reported progress after persistence fails.
    ///
    /// A newly supplied target replaces the saved destination only when the
    /// failure state confirms publication. Otherwise an earlier confirmed
    /// target remains available. `kept` distinguishes released ownership from
    /// a retained resource when the source was published.
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
    /// Records successful cleanup while preserving known publication progress.
    ///
    /// If a destination was confirmed, the publication state remains known;
    /// otherwise cleanup is recorded as releasing an unpublished source.
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
    /// Records a failed cleanup attempt and its effect on source certainty.
    ///
    /// Errors with indeterminate effects make the source state uncertain.
    /// Other errors leave cleanup responsibility outstanding. Previously
    /// confirmed publication information is retained in either case.
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
