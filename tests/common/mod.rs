// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

#[cfg(feature = "async")]
pub(crate) mod internal;
#[allow(dead_code)]
pub(crate) mod poll_support;

#[allow(dead_code)]
pub(crate) mod random_access;
