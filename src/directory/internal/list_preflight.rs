// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- public list_scope_tests exercise the
// shared preflight through both facades.
//! Scope and query validation before provider listing I/O.

use crate::directory::ListFilter;
use crate::directory::ListOptions;
use crate::directory::ListScope;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemProperties;
use crate::path::PathSemantics;

/// Validates the scope and combined raw query prefix without normalizing keys.
pub(crate) fn validate(properties: &FileSystemProperties, scope: &ListScope, options: &ListOptions) -> FsResult<()> {
    let semantics = properties.info().path_semantics();
    match scope {
        ListScope::Path(path) => properties.validate_path(path, FsOperation::List)?,
        ListScope::Namespace if semantics == PathSemantics::Hierarchical => {
            return Err(FsError::new(
                FsErrorKind::InvalidOptions,
                FsOperation::List,
                "hierarchical listing requires a directory path",
            ));
        }
        ListScope::Namespace => {}
    }
    options.validate_for(semantics)?;
    if matches!(semantics, PathSemantics::ObjectKey | PathSemantics::ProviderSpecific) {
        let root_length = scope.path().map_or(0, |path| path.as_str().len());
        let filter_length = match options.filter() {
            Some(ListFilter::LiteralPrefix(prefix)) => prefix.len(),
            _ => 0,
        };
        let length = root_length
            .checked_add(filter_length)
            .and_then(|value| u64::try_from(value).ok());
        if length.is_none_or(|value| properties.limits().max_path_text_bytes().is_exceeded_by(value)) {
            return Err(FsError::new(
                FsErrorKind::ResourceLimitExceeded,
                FsOperation::List,
                "listing query prefix exceeds filesystem path text limit",
            ));
        }
    }
    Ok(())
}
