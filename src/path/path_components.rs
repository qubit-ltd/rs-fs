// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

// qubit-style: allow all -- facade integration tests exercise this API group.
//! Iteration over lexical logical path components.

/// Iterator over the path's lexical component boundaries.
#[derive(Clone, Debug)]
pub struct PathComponents<'a> {
    /// Remaining lexical text with an absolute leading separator removed.
    remaining: Option<&'a str>,
}

impl<'a> PathComponents<'a> {
    /// Creates an iterator for `text`, removing only the absolute root marker.
    pub(crate) fn new(text: &'a str, absolute: bool, literal: bool) -> Self {
        let text = if absolute && !literal {
            text.strip_prefix('/').unwrap_or(text)
        } else {
            text
        };
        Self {
            remaining: (!text.is_empty()).then_some(text),
        }
    }
}

impl<'a> Iterator for PathComponents<'a> {
    type Item = &'a str;

    /// Returns the next lexical component, including empty literal components.
    fn next(&mut self) -> Option<Self::Item> {
        let remaining = self.remaining?;
        match remaining.split_once('/') {
            Some((component, rest)) => {
                self.remaining = Some(rest);
                Some(component)
            }
            None => {
                self.remaining = None;
                Some(remaining)
            }
        }
    }
}
