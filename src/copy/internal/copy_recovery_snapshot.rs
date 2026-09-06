//! Private recovery facts retained by an owning asynchronous copy operation.
use crate::copy::CopyFailureState;
use crate::copy::CopyStats;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CopyRecoverySnapshot {
    pub(crate) state: CopyFailureState,
    pub(crate) stats: CopyStats,
}
impl CopyRecoverySnapshot {
    pub(crate) const fn unchanged() -> Self {
        Self {
            state: CopyFailureState::Unchanged,
            stats: CopyStats {
                files: 0,
                directories: 0,
                symlinks: 0,
                objects: 0,
                prefixes: 0,
                bytes: 0,
                overwritten: 0,
                skipped: 0,
                failed: 0,
            },
        }
    }
}
