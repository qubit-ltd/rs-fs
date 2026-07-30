// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Runtime-neutral provider future type.

use std::future::Future;
use std::pin::Pin;

/// Runtime-neutral boxed future used by asynchronous providers.
///
/// # Type Parameters
/// - `T`: Value produced when the provider operation completes.
pub type SpiFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
