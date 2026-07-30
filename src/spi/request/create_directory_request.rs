// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Validated directory-creation request.

use super::internal::path_request;
use crate::spi::ResolvedCreateDirectoryOptions;

path_request!(CreateDirectoryRequest, ResolvedCreateDirectoryOptions);
