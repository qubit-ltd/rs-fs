// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Facade-resolved option values exposed read-only to providers.

use crate::{
    CopyOptions, CreateDirectoryOptions, DeleteOptions, ListOptions, ReadOptions, RenameOptions,
    WriteOptions,
};

macro_rules! resolved_options {
    ($name:ident, $options:ty) => {
        /// Immutable options resolved by the facade before provider dispatch.
        #[derive(Clone)]
        pub struct $name {
            options: $options,
        }
        impl $name {
            /// Creates this value inside the facade boundary.
            #[allow(dead_code)]
            pub(crate) const fn new(options: $options) -> Self {
                Self { options }
            }
            /// Returns the resolved options.
            #[must_use]
            pub const fn options(&self) -> &$options {
                &self.options
            }
        }
    };
}

resolved_options!(ResolvedReadOptions, ReadOptions);
resolved_options!(ResolvedWriteOptions, WriteOptions);
resolved_options!(ResolvedListOptions, ListOptions);
resolved_options!(ResolvedCreateDirectoryOptions, CreateDirectoryOptions);
resolved_options!(ResolvedDeleteOptions, DeleteOptions);
resolved_options!(ResolvedCopyOptions, CopyOptions);
resolved_options!(ResolvedRenameOptions, RenameOptions);

/// Placeholder resolved persist options; persistence is implemented with temp handles.
pub struct ResolvedPersistOptions;
/// Placeholder resolved temporary-file options; temporary handles are implemented later.
pub struct ResolvedTempFileOptions;
/// Placeholder resolved temporary-directory options; temporary handles are implemented later.
pub struct ResolvedTempDirectoryOptions;
