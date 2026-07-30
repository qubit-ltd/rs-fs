// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Provider-side asynchronous temporary-resource sessions.

use std::pin::Pin;

use super::{
    PersistRequest,
    SpiFuture,
    SpiPersistFailure,
};
use crate::{
    FsResult,
    PersistOutcome,
};

/// Provider-side asynchronous temporary-resource lifecycle session.
pub trait AsyncTempResourceSpi: Send {
    /// Asynchronously confirms provider cleanup.
    fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>>;

    /// Asynchronously releases caller cleanup responsibility.
    fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>>;

    /// Asynchronously persists this resource to a validated target.
    fn persist<'a>(
        self: Pin<&'a mut Self>,
        request: PersistRequest<'a>,
    ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>>;
}
