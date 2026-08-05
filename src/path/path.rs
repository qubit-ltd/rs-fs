// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

//! Provider-neutral logical paths.

use std::fmt::{
    Display,
    Formatter,
    Result as FmtResult,
};

use crate::{
    FsError,
    FsOperation,
    FsResult,
};

use super::{
    PathComponent,
    PathComponents,
    PathSemantics,
    RelativePath,
};

/// A validated logical path independent of any provider-native representation.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct Path {
    /// Whether this logical path starts at a provider root.
    absolute: bool,
    /// Canonical normalized or provider-literal text.
    text: String,
    /// Whether component iteration must preserve literal slash boundaries.
    literal: bool,
    /// Semantics used to validate this spelling.
    semantics: PathSemantics,
}

impl Path {
    /// Creates the canonical hierarchical root path.
    #[inline]
    #[must_use]
    pub fn root() -> Self {
        Self {
            absolute: true,
            text: "/".to_owned(),
            literal: false,
            semantics: PathSemantics::Hierarchical,
        }
    }

    /// Parses a hierarchical logical path using normalized semantics.
    ///
    /// Returns an invalid-path error for empty input, NUL, or root escape.
    #[inline]
    pub fn parse(text: &str) -> FsResult<Self> {
        Self::parse_with_semantics(text, PathSemantics::Hierarchical)
    }

    /// Parses a provider-literal path without interpreting separators or dots.
    ///
    /// Returns an invalid-path error for empty input or NUL.
    #[inline]
    pub fn parse_literal(text: &str) -> FsResult<Self> {
        Self::parse_with_semantics(text, PathSemantics::ObjectKey)
    }

    /// Parses `text` according to explicitly selected provider semantics.
    ///
    /// Hierarchical values normalize empty and dot components and reject root
    /// escapes. Object-key and provider-specific values preserve their text.
    pub fn parse_with_semantics(
        text: &str,
        semantics: PathSemantics,
    ) -> FsResult<Self> {
        if text.is_empty() || text.contains('\0') {
            return Err(invalid_path());
        }
        if semantics != PathSemantics::Hierarchical {
            return Ok(Self {
                absolute: text.starts_with('/'),
                text: text.to_owned(),
                literal: true,
                semantics,
            });
        }
        let absolute = text.starts_with('/');
        let mut components = Vec::new();
        for component in text.split('/') {
            match component {
                "" | "." => {}
                ".." => {
                    if components.pop().is_none() {
                        return Err(invalid_path());
                    }
                }
                value => components.push(value),
            }
        }
        let text = if absolute {
            if components.is_empty() {
                "/".to_owned()
            } else {
                format!("/{}", components.join("/"))
            }
        } else {
            components.join("/")
        };
        if text.is_empty() {
            return Err(invalid_path());
        }
        Ok(Self {
            absolute,
            text,
            literal: false,
            semantics,
        })
    }

    /// Returns the validated logical path text.
    #[inline(always)]
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Returns the final non-empty path component, when one is present.
    ///
    /// A root path and a literal path ending in a separator have no file
    /// name. Hierarchical paths are canonicalized during parsing, so their
    /// final component is always non-empty.
    #[inline(always)]
    #[must_use]
    pub fn file_name(&self) -> Option<&str> {
        if self.text == "/" || (self.literal && self.text.ends_with('/')) {
            return None;
        }
        self.text
            .rsplit('/')
            .find(|component| !component.is_empty())
    }

    /// Returns whether this path is absolute.
    #[inline(always)]
    #[must_use]
    pub const fn is_absolute(&self) -> bool {
        self.absolute
    }

    /// Returns the semantics used to validate this logical path.
    #[inline(always)]
    #[must_use]
    pub const fn semantics(&self) -> PathSemantics {
        self.semantics
    }

    /// Iterates lexical component boundaries without using an empty root value.
    #[inline(always)]
    #[must_use]
    pub fn components(&self) -> PathComponents<'_> {
        PathComponents::new(&self.text, self.absolute, self.literal)
    }

    /// Appends one validated component without re-parsing provider text.
    #[inline(always)]
    #[must_use]
    pub fn child(&self, component: &PathComponent) -> Self {
        self.append(component.as_str())
    }

    /// Appends a safe normalized relative path without re-parsing provider
    /// text.
    #[inline(always)]
    #[must_use]
    pub fn join(&self, relative: &RelativePath) -> Self {
        self.append(relative.as_str())
    }

    /// Joins an already validated suffix to this path.
    fn append(&self, suffix: &str) -> Self {
        let text = if self.text == "/" {
            format!("/{suffix}")
        } else {
            format!("{}/{}", self.text, suffix)
        };
        Self {
            absolute: self.absolute,
            text,
            literal: self.literal,
            semantics: self.semantics,
        }
    }
}

impl Display for Path {
    /// Formats the validated logical spelling.
    #[inline]
    fn fmt(&self, formatter: &mut Formatter<'_>) -> FmtResult {
        formatter.write_str(self.as_str())
    }
}

impl AsRef<str> for Path {
    /// Returns the logical path text for generic text consumers.
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// Builds the shared logical path validation failure.
fn invalid_path() -> FsError {
    FsError::invalid_path(
        FsOperation::ParsePath,
        "path must be non-empty, NUL-free, and remain within its root",
    )
}
