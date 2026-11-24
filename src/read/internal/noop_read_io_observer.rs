// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! No-op observer used by aggregate read methods without observation.

use crate::read::ReadIoObserver;

/// Accepts every provider boundary and discards all I/O events.
pub(crate) struct NoopReadIoObserver;

impl ReadIoObserver for NoopReadIoObserver {}
