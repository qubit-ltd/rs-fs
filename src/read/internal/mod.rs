// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared policy, buffering, and interruption-retry helpers for readers.

mod interrupted_read;
mod random_access_read_policy;
mod read_buffer;

pub(crate) use interrupted_read::read_retry_interrupted;
pub(crate) use random_access_read_policy::RandomAccessReadPolicy;
pub(crate) use read_buffer::ReadBuffer;
