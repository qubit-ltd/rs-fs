// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Observation hooks for asynchronous aggregate reads.

use std::ops::ControlFlow;

use crate::error::FsError;

/// Provider operation being performed by an aggregate read.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadIoPhase {
    /// Opening the reader.
    Open,
    /// Reading a chunk from the reader.
    Read,
}

/// State transition observed while polling a provider operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReadIoEvent {
    /// The provider future has received its first poll.
    Started {
        /// Provider operation that began polling.
        phase: ReadIoPhase,
    },
    /// A provider poll returned pending.
    Pending {
        /// Provider operation whose poll returned pending.
        phase: ReadIoPhase,
    },
    /// A provider poll returned a result.
    Ready {
        /// Provider operation whose poll completed.
        phase: ReadIoPhase,
    },
}

/// Receives actual provider I/O boundaries for asynchronous aggregate reads.
pub trait ReadIoObserver {
    /// Checks whether the next provider operation may begin.
    ///
    /// This is called before each open or read future is created. Returning
    /// `Break(())` stops the operation before that provider call.
    fn before_io(&mut self, _phase: ReadIoPhase) -> ControlFlow<()> {
        ControlFlow::Continue(())
    }

    /// Receives provider-future polling transitions.
    ///
    /// `Started` is emitted on the first actual poll. That same poll also emits
    /// `Ready` or `Pending`; later polls emit only the resulting transition.
    fn on_io_event(&mut self, _event: ReadIoEvent) {}
}

/// Error returned by [`AsyncFileSystem::read_all_observed`] and
/// [`AsyncFileSystem::read_prefix_observed`].
///
/// [`AsyncFileSystem::read_all_observed`]: crate::AsyncFileSystem::read_all_observed
/// [`AsyncFileSystem::read_prefix_observed`]: crate::AsyncFileSystem::read_prefix_observed
#[derive(Debug)]
pub enum ObservedReadError {
    /// The observer stopped the read before its next provider operation.
    Stopped,
    /// The provider or prefix-read policy returned a filesystem error.
    Provider(FsError),
}

impl From<FsError> for ObservedReadError {
    #[inline]
    fn from(error: FsError) -> Self {
        Self::Provider(error)
    }
}
