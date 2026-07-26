// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Filesystem-bound resource handles.

mod async_file_resource;
mod file_resource;

pub use async_file_resource::AsyncFileResource;
pub use file_resource::FileResource;
