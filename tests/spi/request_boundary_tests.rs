// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Compile-time request-boundary coverage is provided by the SPI doctest.

/// Documents that external callers receive requests only from the facade.
#[test]
fn test_request_constructors_are_documented_compile_fail_boundaries() {
    // The compile-fail doctest in `spi::request` verifies this boundary.
}
