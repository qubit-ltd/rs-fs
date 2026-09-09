// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared deterministic lifecycle and validation policy for listing streams.

use std::time::Instant;

use crate::directory::DirectoryStreamState;
use crate::directory::ListOptions;
use crate::directory::ListScope;
use crate::directory::directory_entry_validation;
use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::DirEntry;
use crate::metadata::FileSystemLimits;

/// Runtime-independent policy shared by synchronous and asynchronous handles.
pub(crate) struct ListStreamPolicy {
    /// Validated root constraining returned entries.
    scope: ListScope,
    /// Listing policy used to validate provider results.
    options: ListOptions,
    /// Provider identifier attached to facade-generated errors.
    provider: Box<str>,
    /// Provider path semantics used to validate every returned entry.
    path_semantics: crate::path::PathSemantics,
    /// Provider path limits used to validate every returned entry.
    limits: FileSystemLimits,
    /// Whether enumeration has completed or encountered a terminal failure.
    state: DirectoryStreamState,
    /// Monotonic deadline computed when the stream is created.
    deadline: Option<Instant>,
    /// Number of entries already returned to the caller.
    returned_entries: usize,
}

impl ListStreamPolicy {
    /// Creates a bounded policy using the caller-supplied monotonic instant.
    pub(crate) fn new(
        scope: ListScope,
        options: ListOptions,
        provider: &str,
        path_semantics: crate::path::PathSemantics,
        limits: FileSystemLimits,
        now: Instant,
    ) -> FsResult<Self> {
        let mut policy = Self {
            scope,
            options,
            provider: provider.into(),
            path_semantics,
            limits,
            state: DirectoryStreamState::Open,
            deadline: None,
            returned_entries: 0,
        };
        policy.deadline = policy
            .options
            .deadline()
            .map(|duration| {
                now.checked_add(duration).ok_or_else(|| {
                    policy.contextual_error(FsError::new(
                        FsErrorKind::InvalidOptions,
                        FsOperation::List,
                        "list deadline exceeds the platform monotonic-clock range",
                    ))
                })
            })
            .transpose()?;
        Ok(policy)
    }

    /// Returns the stream's current lifecycle state.
    pub(crate) const fn state(&self) -> DirectoryStreamState {
        self.state
    }

    /// Checks terminal state and deadline immediately before provider dispatch.
    pub(crate) fn before_next(&mut self, now: Instant) -> FsResult<()> {
        if self.state != DirectoryStreamState::Open {
            return Err(self.contextual_error(FsError::new(
                FsErrorKind::InvalidState,
                FsOperation::List,
                "directory stream is terminal",
            )));
        }
        self.check_deadline(now)
    }

    /// Rejects successful late results while retaining actual provider
    /// failures.
    pub(crate) fn finish_next(
        &mut self,
        result: FsResult<Option<DirEntry>>,
        now: Instant,
    ) -> FsResult<Option<DirEntry>> {
        let entry = match result {
            Ok(entry) => entry,
            Err(error) => {
                self.state = DirectoryStreamState::Failed;
                return Err(self.contextual_error(error));
            }
        };
        self.check_deadline(now)?;
        match entry {
            Some(entry) => {
                if let Err(error) =
                    directory_entry_validation::validate_entry(&entry, &self.scope, self.path_semantics, self.limits)
                {
                    self.state = DirectoryStreamState::Failed;
                    return Err(self.contextual_error(error));
                }
                if let Err(message) =
                    crate::directory::internal::select(&entry, &self.scope, &self.options, self.path_semantics)
                {
                    self.state = DirectoryStreamState::Failed;
                    return Err(self.contextual_error(directory_entry_validation::option_error(&self.scope, message)));
                }
                if self.options.max_depth().is_some_and(|maximum| {
                    self.scope
                        .path()
                        .and_then(|root| directory_entry_validation::entry_depth(root, &entry.path))
                        .is_some_and(|depth| depth > maximum)
                }) {
                    self.state = DirectoryStreamState::Failed;
                    return Err(self.resource_limit_error("directory listing depth limit was exceeded"));
                }
                if self
                    .options
                    .max_entries()
                    .is_some_and(|maximum| self.returned_entries >= maximum)
                {
                    self.state = DirectoryStreamState::Failed;
                    return Err(self.resource_limit_error("directory listing entry limit was exceeded"));
                }
                self.returned_entries = self.returned_entries.checked_add(1).ok_or_else(|| {
                    self.state = DirectoryStreamState::Failed;
                    self.resource_limit_error("directory listing entry count exceeded the API range")
                })?;
                Ok(Some(entry))
            }
            None => {
                self.state = DirectoryStreamState::Exhausted;
                Ok(None)
            }
        }
    }

    /// Marks successful results at or after the inclusive deadline as failed.
    fn check_deadline(&mut self, now: Instant) -> FsResult<()> {
        if self.deadline.is_some_and(|deadline| now >= deadline) {
            self.state = DirectoryStreamState::Failed;
            return Err(self.resource_limit_error("directory listing deadline was exceeded"));
        }
        Ok(())
    }

    /// Adds only missing facade facts to a provider stream error.
    fn contextual_error(&self, error: FsError) -> FsError {
        let error = error
            .with_operation(FsOperation::List)
            .with_missing_provider(&self.provider);
        match self.scope.path() {
            Some(path) => error.with_missing_context(path, None, &self.provider),
            None => error,
        }
    }

    /// Builds a terminal caller-budget error with list context.
    fn resource_limit_error(&self, message: &str) -> FsError {
        self.contextual_error(FsError::new(
            FsErrorKind::ResourceLimitExceeded,
            FsOperation::List,
            message,
        ))
    }
}
