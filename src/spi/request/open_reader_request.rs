// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Validated reader-open request.
//! The facade routes this request through `FileSystemSpi::open_reader` after
//! validating the path and resolving read options.

use super::internal::path_request;
use crate::spi::ResolvedReadOptions;

path_request!(OpenReaderRequest, ResolvedReadOptions);
