// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Private implementation details for copy orchestration.

mod copy_cancellation_guard;

pub(super) use copy_cancellation_guard::CopyCancellationGuard;
