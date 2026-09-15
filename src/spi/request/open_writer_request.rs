// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// facade.
//! Validated writer-open request.

use super::internal::path_request;
use crate::spi::ResolvedWriteOptions;

path_request!(OpenWriterRequest, ResolvedWriteOptions);
