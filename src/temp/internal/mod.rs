// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared private temporary-resource lifecycle rules.
mod temp_lifecycle;
pub(crate) use temp_lifecycle::TempLifecycle;
mod sync_temp_resource;
pub(crate) use sync_temp_resource::SyncTempResource;
