// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Validated metadata request.
//! The facade routes this request through `FileSystemSpi::stat` after
//! validating the path; the request carries no operation-specific options.

use super::internal::path_request;

path_request!(StatRequest, ());
