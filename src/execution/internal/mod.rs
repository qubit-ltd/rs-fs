// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared implementation details for execution adapters.

mod prepared_read_lifecycle;
pub(super) use prepared_read_lifecycle::PreparedReadLifecycle;
pub(super) use prepared_read_lifecycle::bind_prepared;
pub(super) use prepared_read_lifecycle::take_unclaimed_request;
