// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Definition helper for facade-resolved option values.

/// Defines one immutable facade-resolved option value.
macro_rules! resolved_options {
    ($name:ident, $options:ty) => {
        /// Immutable options resolved by the facade before provider dispatch.
        #[derive(Clone)]
        pub struct $name {
            /// Caller options retained after facade validation and
            /// normalization.
            options: $options,
        }

        impl $name {
            /// Creates this value inside the facade boundary.
            ///
            /// # Parameters
            /// - `options`: Caller options after facade validation and normalization.
            ///
            /// # Returns
            /// An immutable provider-facing option envelope.
            #[allow(dead_code)]
            #[inline]
            pub(crate) const fn new(options: $options) -> Self {
                Self { options }
            }

            /// Returns the resolved options.
            ///
            /// # Returns
            /// The validated options retained by this envelope.
            #[inline(always)]
            #[must_use]
            pub const fn options(&self) -> &$options {
                &self.options
            }
        }
    };
}

pub(in crate::spi) use resolved_options;
