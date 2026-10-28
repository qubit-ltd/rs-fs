// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Local failure guard for an interrupted asynchronous list request.

use std::time::Instant;

use crate::directory::ListStreamPolicy;
use crate::error::FsResult;
use crate::metadata::DirEntry;

/// Marks the policy failed if a started provider request is dropped pending.
pub(crate) struct ListNextCancellationGuard<'a> {
    /// Policy whose request lifecycle this guard protects.
    policy: &'a mut ListStreamPolicy,
    /// Whether the provider result was completed and recorded.
    finished: bool,
}

impl<'a> ListNextCancellationGuard<'a> {
    /// Starts guarding a request after policy preflight has succeeded.
    pub(crate) fn new(policy: &'a mut ListStreamPolicy) -> Self {
        Self {
            policy,
            finished: false,
        }
    }

    /// Records the completed provider result and disables cancellation failure.
    pub(crate) fn finish(mut self, result: FsResult<Option<DirEntry>>, now: Instant) -> FsResult<Option<DirEntry>> {
        let outcome = self.policy.finish_next(result, now);
        self.finished = true;
        outcome
    }
}

impl Drop for ListNextCancellationGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.policy.mark_cancelled();
        }
    }
}
