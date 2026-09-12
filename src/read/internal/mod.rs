// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
// =============================================================================
//! Shared bounded allocation policy for aggregate readers.

mod interrupted_read;
mod read_buffer;

pub(crate) use interrupted_read::read_retry_interrupted;
pub(crate) use read_buffer::ReadBuffer;
