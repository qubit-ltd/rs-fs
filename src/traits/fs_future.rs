// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Object-safe filesystem future type.

use std::future::Future;
use std::pin::Pin;

use crate::FsResult;

/// Boxed, sendable future used by object-safe asynchronous filesystem APIs.
pub type FsFuture<'a, T> =
    Pin<Box<dyn Future<Output = FsResult<T>> + Send + 'a>>;
