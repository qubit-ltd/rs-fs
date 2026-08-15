// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Shared immutable state and deterministic policy for filesystem facades.

use std::sync::Arc;

use qubit_budget::InsufficientBudgetError;
use qubit_budget::ResourceBudget;

use crate::error::FsError;
use crate::error::FsErrorKind;
use crate::error::FsOperation;
use crate::error::FsResult;
use crate::metadata::FileSystemCapability;
use crate::metadata::FileSystemProperties;
use crate::path::Path;
use crate::spi::ProviderOperation;
use crate::spi::ProviderOperations;
use crate::spi::ProviderProperties;

mod resource_budget {
    use qubit_budget::ResourceBudget;

    /// Resources counted by filesystem facade byte budgets.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum FileSystemResource {
        /// Bytes read from a source.
        ReadBytes,
        /// Bytes accepted by a destination writer.
        WriteBytes,
    }

    /// A budget that counts filesystem bytes.
    pub(crate) type ByteBudget = ResourceBudget<FileSystemResource, u64>;
}

pub(crate) use resource_budget::ByteBudget;
pub(crate) use resource_budget::FileSystemResource;

/// Immutable provider state and deterministic preflight shared by facades.
pub(crate) struct FacadeCore {
    /// Effective application-facing properties derived from the provider.
    properties: Arc<FileSystemProperties>,
    /// Concrete provider operations captured with the property snapshot.
    provider_operations: ProviderOperations,
}

impl FacadeCore {
    /// Fixed I/O chunk used by bounded prefix reads.
    pub(crate) const PREFIX_BUFFER_SIZE: usize = 8192;

    /// Validates and captures one provider property snapshot.
    ///
    /// # Errors
    /// Returns the provider-property validation error when the declared
    /// capabilities and concrete operations are inconsistent.
    pub(crate) fn new(provider: ProviderProperties) -> FsResult<Self> {
        let provider_operations = provider.operations();
        let properties = FileSystemProperties::from_provider(&provider)?;
        Ok(Self {
            properties: Arc::new(properties),
            provider_operations,
        })
    }

    /// Returns the cached effective application-facing properties.
    #[inline(always)]
    pub(crate) fn properties(&self) -> &FileSystemProperties {
        &self.properties
    }

    /// Reports whether the captured provider exposes a concrete operation.
    #[inline(always)]
    pub(crate) fn provider_supports(
        &self,
        operation: ProviderOperation,
    ) -> bool {
        self.provider_operations.supports(operation)
    }

    /// Validates one logical path against the cached provider snapshot.
    ///
    /// # Errors
    /// Returns an enriched invalid-path or resource-limit error when the path
    /// semantics, form, constraints, or limits do not match the filesystem.
    pub(crate) fn validate_path(
        &self,
        path: &Path,
        operation: FsOperation,
    ) -> FsResult<()> {
        if path.semantics() != self.properties.info().path_semantics() {
            return Err(FsError::invalid_path(
                operation,
                "path semantics do not match this filesystem",
            )
            .with_path(path.clone())
            .with_provider(self.properties.info().provider_id()));
        }
        self.properties
            .path_constraints()
            .validate(path)
            .map_err(|error| self.enrich(error, Some(path), operation))?;
        self.properties
            .limits()
            .validate_path(
                path,
                self.properties.info().path_semantics(),
                operation,
            )
            .map_err(|error| self.enrich(error, Some(path), operation))
    }

    /// Validates the optional parent used for temporary resource creation.
    ///
    /// # Errors
    /// Returns an enriched path-validation error when `parent` is present and
    /// does not satisfy the cached filesystem rules.
    pub(crate) fn validate_temp_parent(
        &self,
        parent: Option<&Path>,
    ) -> FsResult<()> {
        parent.map_or(Ok(()), |path| {
            self.validate_path(path, FsOperation::CreateTemp)
        })
    }

    /// Requires one capability before an operation can create provider I/O.
    ///
    /// `path` may be absent for operations that have no natural path context.
    ///
    /// # Errors
    /// Returns an enriched unsupported-capability error when the cached
    /// properties do not advertise `capability`.
    pub(crate) fn require(
        &self,
        capability: FileSystemCapability,
        operation: FsOperation,
        path: Option<&Path>,
    ) -> FsResult<()> {
        if self.properties.capabilities().supports(capability) {
            Ok(())
        } else {
            let error = FsError::new(
                FsErrorKind::UnsupportedCapability,
                operation,
                "filesystem capability is not supported",
            )
            .with_required_capability(capability)
            .with_missing_provider(self.properties.info().provider_id());
            Err(match path {
                Some(path) => error.with_path(path.clone()),
                None => error,
            })
        }
    }

    /// Adds missing operation, optional path, and provider context to an error.
    ///
    /// Existing provider-supplied context is preserved. `None` does not invent
    /// a path for pathless operations.
    pub(crate) fn enrich(
        &self,
        error: FsError,
        path: Option<&Path>,
        operation: FsOperation,
    ) -> FsError {
        let error = error
            .with_operation(operation)
            .with_missing_provider(self.properties.info().provider_id());
        match path {
            Some(path) => error.with_missing_context(
                path,
                None,
                self.properties.info().provider_id(),
            ),
            None => error,
        }
    }

    /// Builds a provider-contract error bound to a requested path.
    pub(crate) fn contract_error(
        &self,
        path: &Path,
        operation: FsOperation,
        message: &str,
    ) -> FsError {
        FsError::new(FsErrorKind::ProviderContractViolation, operation, message)
            .with_path(path.clone())
            .with_provider(self.properties.info().provider_id())
    }

    /// Creates a byte budget with the supplied inclusive limit.
    #[inline]
    pub(crate) fn byte_budget(
        resource: FileSystemResource,
        maximum: u64,
    ) -> ByteBudget {
        ResourceBudget::new(resource, maximum)
    }

    /// Converts a platform-sized I/O count to the budget quantity type.
    ///
    /// # Errors
    /// Returns a contextual resource-limit error when `value` cannot be
    /// represented by the public `u64` accounting domain.
    #[inline]
    pub(crate) fn quantity_from_usize(
        value: usize,
        operation: FsOperation,
        path: &Path,
        provider: &str,
    ) -> Result<u64, FsError> {
        u64::try_from(value).map_err(|error| {
            FsError::with_source(
                FsErrorKind::ResourceLimitExceeded,
                operation,
                "I/O byte count cannot be represented by the resource budget",
                error,
            )
            .with_path(path.clone())
            .with_provider(provider)
        })
    }

    /// Converts a budget failure into a contextual filesystem error.
    #[inline]
    pub(crate) fn budget_error(
        error: InsufficientBudgetError<FileSystemResource, u64>,
        operation: FsOperation,
        path: &Path,
        provider: &str,
        message: &'static str,
    ) -> FsError {
        FsError::with_source(
            FsErrorKind::ResourceLimitExceeded,
            operation,
            message,
            error,
        )
        .with_path(path.clone())
        .with_provider(provider)
    }

    /// Returns the next bounded read length for an accumulated prefix.
    #[inline(always)]
    pub(crate) fn next_prefix_read_len(
        accumulated: usize,
        maximum: usize,
    ) -> usize {
        maximum
            .saturating_sub(accumulated)
            .min(Self::PREFIX_BUFFER_SIZE)
    }
}
