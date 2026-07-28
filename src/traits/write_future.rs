// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Object-safe futures for typed write publication results.

use std::future::Future;
use std::pin::Pin;

use crate::{
    WriteFailure,
    WriteOutcome,
};

/// Boxed asynchronous result of publishing a file write session.
pub type WriteFuture<'a> = Pin<
    Box<dyn Future<Output = Result<WriteOutcome, WriteFailure>> + Send + 'a>,
>;
