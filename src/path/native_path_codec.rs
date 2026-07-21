// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Native path text codec abstraction.

use std::borrow::Cow;
use std::error::Error;

/// Converts between canonical UTF-8 path text and a provider-native string.
///
/// This trait operates on opaque string fragments only. It does not split path
/// components, interpret separators, normalize `.` or `..`, process roots, or
/// parse URI encoding. Hierarchical providers normally invoke it for each
/// component, while object-key providers may invoke it for one complete key.
///
/// Canonical text keeps ordinary Unicode unchanged, encodes a literal percent
/// sign as `%25`, and encodes control characters and non-UTF-8 native bytes as
/// uppercase `%XX` byte escapes. Consequently, within a codec's supported
/// domain, `encode(decode(native)) == native` and
/// `decode(encode(canonical_text)) == canonical_text`.
///
/// The canonical encoder proceeds left to right through native bytes. A valid
/// UTF-8 scalar is copied unless it is `%` or a control character; those cases
/// emit an escape for each UTF-8 byte. A byte that cannot begin a valid UTF-8
/// scalar is emitted as one `%XX` escape. It performs neither Unicode
/// normalization nor case conversion. Therefore `%25`, `%0A`, and `%80` can
/// be canonical, whereas a raw `%`, `%2f`, `%41`, and `%E4%B8%AD` are rejected
/// as malformed or non-canonical aliases.
///
/// Implementations should return [`Cow::Borrowed`] for plain representable
/// UTF-8 text when no escaping or representation conversion is required. Error
/// offsets reported by the built-in codec error are UTF-8 byte offsets for text,
/// native byte offsets for byte input, and WTF-8 byte offsets for Windows
/// surrogate representations.
pub trait NativePathCodec: Send + Sync {
    /// Borrowed native string representation used by the provider.
    type Native: ?Sized + ToOwned;

    /// Error returned when either representation is invalid or unsupported.
    type Error: Error + Send + Sync + 'static;

    /// Encodes canonical UTF-8 path text into the native representation.
    ///
    /// # Errors
    ///
    /// Returns an error when `text` is not canonical or cannot be represented
    /// by the selected native encoding.
    fn encode<'a>(&self, text: &'a str) -> Result<Cow<'a, Self::Native>, Self::Error>;

    /// Decodes a native path representation into canonical UTF-8 text.
    ///
    /// # Errors
    ///
    /// Returns an error when `native` is invalid for the selected encoding or
    /// cannot be represented without loss.
    fn decode<'a>(&self, native: &'a Self::Native) -> Result<Cow<'a, str>, Self::Error>;
}
