// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Poll state transitions reported for provider operations.

use crate::read::ReadIoPhase;

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
