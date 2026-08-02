// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Copy facade integration tests.

#[cfg(feature = "async")]
mod async_copy_fallback_tests;
#[cfg(feature = "async")]
mod async_copy_operation_tests;
mod copy_fallback_tests;
