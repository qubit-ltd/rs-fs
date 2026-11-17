// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private state helpers for managed read ports.

mod managed_read_state;

pub(in crate::spi::managed_read) use managed_read_state::ManagedReadState;
