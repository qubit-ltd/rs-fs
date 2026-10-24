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
/// The future is pinned and must be `Send`, allowing provider operations to
/// cross thread boundaries while borrowing data for `'a`.
///
/// # Type Parameters
/// - `T`: Value produced when the provider operation completes.
///
/// # Lifetimes
/// - `'a`: Maximum lifetime of borrows held by the future.
///
/// # Examples
///
/// ```
/// use qubit_fs::spi::SpiFuture;
///
/// async fn provider_operation() -> usize {
///     let future: SpiFuture<'static, usize> = Box::pin(async { 42 });
///     future.await
/// }
///
/// let _operation = provider_operation();
/// ```
pub type SpiFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
