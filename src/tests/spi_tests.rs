// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Tests for provider-facing SPI contracts.

mod resolved_copy_options_tests;
mod resolved_list_options_tests;

#[test]
fn test_resolved_read_options_preserve_options_and_prefix_hint() {
    use crate::read::ReadOptions;
    use crate::spi::PrefixReadHint;
    use crate::spi::ResolvedReadOptions;

    let options = ReadOptions::default().with_length(Some(42));
    let resolved = ResolvedReadOptions::new(options.clone());
    assert_eq!(resolved.options(), &options);
    assert_eq!(resolved.prefix_hint(), None);
    let hinted = resolved.with_prefix_hint(PrefixReadHint::new(12));
    assert_eq!(hinted.options(), &options);
    assert_eq!(hinted.prefix_hint().unwrap().max_bytes(), 12);
}
