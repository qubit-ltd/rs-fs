// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Object-safe future for typed persistence results.

use std::future::Future;
use std::pin::Pin;

use crate::{
    PersistFailure,
    PersistOutcome,
};

/// Boxed sendable future resolving to a typed temporary persistence result.
pub type PersistFuture<'a> = Pin<
    Box<
        dyn Future<Output = Result<PersistOutcome, PersistFailure>> + Send + 'a,
    >,
>;
