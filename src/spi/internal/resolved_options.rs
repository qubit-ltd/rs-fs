// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Definition helper for facade-resolved option values.

/// Defines one immutable facade-resolved option value.
macro_rules! resolved_options {
    ($name:ident, $options:ty) => {
        #[doc = concat!(
                                            "Immutable options resolved by the facade before provider dispatch.\n\n",
                                            "The facade places this value in the request passed to the provider's corresponding `FileSystemSpi` operation.\n\n",
                                            "# Examples\n\n",
                                            "```rust\n",
                                            "use ", stringify!(qubit_fs), "::spi::", stringify!($name), ";\n\n",
                                            "fn inspect(options: &", stringify!($name), ") {\n",
                                            "    let _caller_options = options.options();\n",
                                            "}\n",
                                            "```",
                                        )]
        #[must_use = "pass the resolved options to the provider"]
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
            #[inline]
            #[must_use = "inspect the caller options before provider dispatch"]
            pub const fn options(&self) -> &$options {
                &self.options
            }
        }
    };
}

pub(in crate::spi) use resolved_options;
