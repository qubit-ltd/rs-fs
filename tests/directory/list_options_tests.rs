// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::hint::black_box;
use std::time::Duration;

use qubit_fs::directory::ListFilter;
use qubit_fs::directory::ListOptions;
use qubit_fs::metadata::SymlinkPolicy;

#[test]
fn test_option_accessors_are_executed_at_runtime() {
    let constructor: fn() -> ListOptions = black_box(Default::default);
    let with_recursive: fn(ListOptions, bool) -> ListOptions = black_box(ListOptions::with_recursive);
    let recursive: fn(&ListOptions) -> bool = black_box(ListOptions::recursive);
    let with_symlink_policy: fn(ListOptions, SymlinkPolicy) -> ListOptions =
        black_box(ListOptions::with_symlink_policy);
    let symlink_policy_override: fn(&ListOptions) -> Option<SymlinkPolicy> =
        black_box(ListOptions::symlink_policy_override);
    let with_include_metadata: fn(ListOptions, bool) -> ListOptions = black_box(ListOptions::with_include_metadata);
    let include_metadata: fn(&ListOptions) -> bool = black_box(ListOptions::include_metadata);
    let with_page_size: fn(ListOptions, Option<usize>) -> ListOptions = black_box(ListOptions::with_page_size);
    let page_size: fn(&ListOptions) -> Option<usize> = black_box(ListOptions::page_size);
    let with_prefix: fn(ListOptions, Option<String>) -> ListOptions = black_box(ListOptions::with_prefix);
    let prefix: for<'a> fn(&'a ListOptions) -> Option<&'a str> = black_box(ListOptions::prefix);
    let with_filter: fn(ListOptions, Option<ListFilter>) -> ListOptions = black_box(ListOptions::with_filter);
    let filter: for<'a> fn(&'a ListOptions) -> Option<&'a ListFilter> = black_box(ListOptions::filter);
    let object_keys: fn() -> ListOptions = black_box(ListOptions::object_keys);
    let with_max_depth: fn(ListOptions, Option<usize>) -> ListOptions = black_box(ListOptions::with_max_depth);
    let max_depth: fn(&ListOptions) -> Option<usize> = black_box(ListOptions::max_depth);
    let with_max_entries: fn(ListOptions, Option<usize>) -> ListOptions = black_box(ListOptions::with_max_entries);
    let max_entries: fn(&ListOptions) -> Option<usize> = black_box(ListOptions::max_entries);
    let with_deadline: fn(ListOptions, Option<Duration>) -> ListOptions = black_box(ListOptions::with_deadline);
    let deadline: fn(&ListOptions) -> Option<Duration> = black_box(ListOptions::deadline);

    let options = with_deadline(
        with_max_entries(
            with_max_depth(
                with_filter(
                    with_prefix(
                        with_page_size(
                            with_include_metadata(
                                with_symlink_policy(with_recursive(constructor(), true), SymlinkPolicy::Reject),
                                true,
                            ),
                            Some(20),
                        ),
                        Some("reports".to_owned()),
                    ),
                    Some(ListFilter::Subtree("reports".to_owned())),
                ),
                Some(2),
            ),
            Some(10),
        ),
        Some(Duration::from_secs(1)),
    );

    assert!(recursive(&options));
    assert_eq!(Some(SymlinkPolicy::Reject), symlink_policy_override(&options));
    assert!(include_metadata(&options));
    assert_eq!(Some(20), page_size(&options));
    assert_eq!(Some("reports"), prefix(&options));
    assert!(matches!(filter(&options), Some(ListFilter::Subtree(value)) if value == "reports"));
    assert!(recursive(&object_keys()));
    assert_eq!(Some(2), max_depth(&options));
    assert_eq!(Some(10), max_entries(&options));
    assert_eq!(Some(Duration::from_secs(1)), deadline(&options));
}
