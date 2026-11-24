// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Observer callbacks for asynchronous aggregate reads.

use std::ops::ControlFlow;

use crate::read::ReadIoEvent;
use crate::read::ReadIoPhase;

/// Receives actual provider I/O boundaries for asynchronous aggregate reads.
pub trait ReadIoObserver {
    /// Checks whether the next provider operation may begin.
    ///
    /// This is called before each open or read future is created. Returning
    /// `Break(())` stops the operation before that provider call.
    ///
    /// # Parameters
    /// - `phase`: Provider operation about to be started.
    ///
    /// # Returns
    /// `Continue(())` to proceed or `Break(())` to stop the aggregate read.
    fn before_io(&mut self, phase: ReadIoPhase) -> ControlFlow<()> {
        let _ = phase;
        ControlFlow::Continue(())
    }

    /// Receives provider-future polling transitions.
    ///
    /// `Started` is emitted on the first actual poll. That same poll also emits
    /// `Ready` or `Pending`; later polls emit only the resulting transition.
    ///
    /// # Parameters
    /// - `event`: Poll transition from the provider operation.
    fn on_io_event(&mut self, event: ReadIoEvent) {
        let _ = event;
    }
}
