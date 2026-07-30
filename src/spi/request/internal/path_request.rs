// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Definition helper for single-path provider requests.

/// Defines one facade-created request with a validated logical path.
macro_rules! path_request {
    ($name:ident, $options:ty) => {
        /// A facade-created request with a validated logical path.
        pub struct $name<'a> {
            path: &'a crate::Path,
            options: $options,
        }

        impl<'a> $name<'a> {
            /// Creates this request inside the facade boundary.
            #[inline(always)]
            pub(crate) const fn new(
                path: &'a crate::Path,
                options: $options,
            ) -> Self {
                Self { path, options }
            }

            /// Returns the validated logical path.
            #[inline(always)]
            #[must_use]
            pub const fn path(&self) -> &'a crate::Path {
                self.path
            }

            /// Returns the resolved operation options.
            #[inline(always)]
            #[must_use]
            pub const fn options(&self) -> &$options {
                &self.options
            }
        }
    };
}

pub(in crate::spi::request) use path_request;
