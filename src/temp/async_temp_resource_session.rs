// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Provider-side asynchronous temporary resource lifecycle sessions.

use std::pin::Pin;

use crate::{
    FsFuture,
    FsPath,
    PersistFuture,
    PersistOptions,
};

/// Provider lifecycle session underlying asynchronous temp handles.
pub trait AsyncTempResourceSession: Send {
    /// Asynchronously deletes the source and staging resources.
    fn cleanup_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()>;

    /// Asynchronously transfers cleanup responsibility to the caller.
    fn keep_async<'a>(self: Pin<&'a mut Self>) -> FsFuture<'a, ()>;

    /// Asynchronously publishes the source with typed partial progress.
    fn persist_async<'a>(
        self: Pin<&'a mut Self>,
        target: &'a FsPath,
        options: PersistOptions,
    ) -> PersistFuture<'a>;

    /// Performs nonblocking local cancellation when an async handle is dropped.
    ///
    /// Implementations must not create or block an executor and must not claim
    /// that remote cleanup completed.
    #[inline]
    fn cancel_on_drop(self: Pin<&mut Self>) {}
}
