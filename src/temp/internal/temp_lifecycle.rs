//! Private temporary-resource recovery snapshot and transitions.
use crate::error::FsError;
use crate::path::Path;
use crate::temp::PersistFailureState;
use crate::temp::TempResourceState;

#[derive(Clone)]
pub(crate) struct TempLifecycle {
    state: TempResourceState,
    failure_state: PersistFailureState,
    publication_target: Option<Path>,
}
impl TempLifecycle {
    pub(crate) const fn new() -> Self {
        Self {
            state: TempResourceState::Owned,
            failure_state: PersistFailureState::NotPublished,
            publication_target: None,
        }
    }
    pub(crate) const fn state(&self) -> TempResourceState {
        self.state
    }
    pub(crate) const fn failure_state(&self) -> PersistFailureState {
        self.failure_state
    }
    pub(crate) fn publication_target(&self) -> Option<&Path> {
        self.publication_target.as_ref()
    }
    pub(crate) fn begin_pending(&mut self) {
        self.state = TempResourceState::Indeterminate;
        self.failure_state = PersistFailureState::Indeterminate;
    }
    pub(crate) fn record_success(&mut self, kept: bool, target: Path) {
        self.state = if kept {
            TempResourceState::Kept
        } else {
            TempResourceState::Persisted
        };
        self.failure_state = PersistFailureState::PublishedSourceReleased;
        self.publication_target = Some(target);
    }
    pub(crate) fn record_failure(&mut self, state: PersistFailureState, target: Option<Path>, kept: bool) {
        if let Some(target) = target {
            self.publication_target = Some(target);
        }
        self.failure_state = state;
        self.state = match state {
            PersistFailureState::NotPublished => TempResourceState::Owned,
            PersistFailureState::NotPublishedSourceReleased => TempResourceState::Cleaned,
            PersistFailureState::PublishedSourceRetained => TempResourceState::CleanupRequired,
            PersistFailureState::PublishedSourceReleased => {
                if kept {
                    TempResourceState::Kept
                } else {
                    TempResourceState::Persisted
                }
            }
            PersistFailureState::Indeterminate => TempResourceState::Indeterminate,
        };
    }
    pub(crate) fn record_cleanup_success(&mut self) {
        self.state = TempResourceState::Cleaned;
        self.failure_state = match self.failure_state {
            PersistFailureState::PublishedSourceRetained | PersistFailureState::PublishedSourceReleased => {
                PersistFailureState::PublishedSourceReleased
            }
            PersistFailureState::Indeterminate => PersistFailureState::Indeterminate,
            _ => PersistFailureState::NotPublishedSourceReleased,
        };
    }
    pub(crate) fn record_cleanup_error(&mut self, error: &FsError) {
        if error.has_indeterminate_effect() {
            self.begin_pending();
        } else {
            self.state = TempResourceState::CleanupRequired;
        }
    }

    #[allow(dead_code)]
    pub(crate) fn restore_state(&mut self, state: TempResourceState) {
        self.state = state;
    }
}
