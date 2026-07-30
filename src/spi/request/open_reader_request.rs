// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated reader-open request.

use super::internal::path_request;
use crate::spi::ResolvedReadOptions;

path_request!(OpenReaderRequest, ResolvedReadOptions);
