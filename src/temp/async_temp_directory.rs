// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
// qubit-style: allow source-test-pair -- behavior is covered through the public
// facade.
//! Runtime-neutral asynchronous temporary-directory facade handle.

use crate::AsyncFileSystem;
use crate::error::FsResult;
use crate::path::Path;
use crate::path::PathComponent;
use crate::path::RelativePath;
use crate::spi::AsyncTempResourceSpi;
use crate::spi::SpiFuture;
use crate::temp::AsyncTempFile;
use crate::temp::PersistFailure;
use crate::temp::PersistOptions;
use crate::temp::PersistOutcome;
use crate::temp::TempResourceState;

/// A facade-owned asynchronous temporary directory.
///
/// # Examples
///
/// ```rust
/// # mod support { include!(concat!(env!("CARGO_MANIFEST_DIR"), "/tests/common/rustdoc_support.rs")); }
/// # use support::*;
/// # let (filesystem, _) = async_recording_spi::async_recording_file_system(Default::default());
/// # poll_support::ready(async {
/// use qubit_fs::temp::{TempOptions, TempResourceState};
///
/// let mut temporary = filesystem.create_temp_directory(TempOptions::default()).await?;
/// temporary.cleanup().await?;
/// assert_eq!(TempResourceState::Cleaned, temporary.state());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// # }).unwrap();
/// ```
pub struct AsyncTempDirectory(
    /// Shared asynchronous temporary-resource lifecycle implementation.
    AsyncTempFile,
);

impl AsyncTempDirectory {
    /// Binds a validated provider temporary-directory session to its facade.
    ///
    /// # Parameters
    /// - `file_system`: Facade that owns validation and persistence policy.
    /// - `path`: Validated provider-local temporary path.
    /// - `session`: Provider lifecycle session.
    ///
    /// # Returns
    /// An owned asynchronous temporary-directory handle.
    #[inline]
    pub(crate) fn new(file_system: AsyncFileSystem, path: Path, session: Box<dyn AsyncTempResourceSpi>) -> Self {
        Self(AsyncTempFile::new(file_system, path, session, "temporary directory"))
    }

    /// Returns the provider-local temporary path.
    ///
    /// # Returns
    /// The validated path supplied by the provider.
    #[inline]
    #[must_use]
    pub const fn path(&self) -> &Path {
        self.0.path()
    }

    /// Returns the current ownership lifecycle state.
    ///
    /// # Returns
    /// The handle's current cleanup and publication state.
    #[inline]
    #[must_use]
    pub const fn state(&self) -> TempResourceState {
        self.0.state()
    }

    /// Returns one lexically safe child path.
    #[inline]
    #[must_use]
    pub fn child(&self, component: &PathComponent) -> Path {
        self.0.child(component)
    }

    /// Returns one lexically safe descendant path.
    #[inline]
    #[must_use]
    pub fn descendant(&self, relative: &RelativePath) -> Path {
        self.0.descendant(relative)
    }

    /// Asynchronously confirms cleanup of this temporary directory.
    ///
    /// # Returns
    /// A future resolving after provider cleanup is confirmed.
    ///
    /// # Errors
    /// Resolves to an invalid-state error when cleanup is no longer legal, or
    /// to the provider cleanup failure.
    #[inline]
    pub fn cleanup(&mut self) -> SpiFuture<'_, FsResult<()>> {
        self.0.cleanup()
    }

    /// Asynchronously publishes this directory to a generated target.
    ///
    /// # Returns
    /// A future resolving to the confirmed publication outcome.
    ///
    /// # Errors
    /// Resolves to an invalid-state error when the directory is no longer
    /// owned, or to the provider ownership-transfer failure.
    #[inline]
    pub fn keep(&mut self) -> SpiFuture<'_, Result<PersistOutcome, PersistFailure>> {
        self.0.keep()
    }

    /// Asynchronously persists this directory to a validated destination.
    ///
    /// # Parameters
    /// - `target`: Validated destination path.
    /// - `options`: Persistence atomicity and publication requirements.
    ///
    /// # Returns
    /// A future resolving to the confirmed persistence outcome.
    ///
    /// # Errors
    /// Resolves to a typed failure for invalid lifecycle state, failed local
    /// preflight, provider failure, or provider contract violation.
    #[inline]
    pub fn persist<'a>(
        &'a mut self,
        target: &'a Path,
        options: PersistOptions,
    ) -> SpiFuture<'a, Result<PersistOutcome, PersistFailure>> {
        self.0.persist(target, options)
    }
}

#[cfg(test)]
mod tests {
    use std::pin::Pin;

    use super::AsyncTempDirectory;
    use crate::AsyncFileSystem;
    use crate::error::FsResult;
    use crate::metadata::FileKind;
    use crate::metadata::FileSystemCapabilities;
    use crate::metadata::FileSystemCapability;
    use crate::metadata::FileSystemId;
    use crate::metadata::FileSystemInfo;
    use crate::metadata::FileSystemLimits;
    use crate::metadata::SymlinkPolicy;
    use crate::path::Path;
    use crate::path::PathComponent;
    use crate::path::PathConstraints;
    use crate::path::PathSemantics;
    use crate::path::RelativePath;
    use crate::spi::AsyncFileSystemSpi;
    use crate::spi::AsyncTempResourceSpi;
    use crate::spi::PersistRequest;
    use crate::spi::ProviderProperties;
    use crate::spi::SpiFuture;
    use crate::spi::SpiPersistFailure;
    use crate::spi::StatRequest;
    use crate::spi::StatResponse;
    use crate::temp::PersistOptions;
    use crate::temp::PersistOutcome;
    use crate::temp::TempResourceState;

    struct Session;

    impl AsyncTempResourceSpi for Session {
        fn cleanup<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, FsResult<()>> {
            Box::pin(async { panic!("test future is not polled") })
        }

        fn keep<'a>(self: Pin<&'a mut Self>) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
            Box::pin(async { panic!("test future is not polled") })
        }

        fn persist<'a>(
            self: Pin<&'a mut Self>,
            _: PersistRequest<'a>,
        ) -> SpiFuture<'a, Result<PersistOutcome, SpiPersistFailure>> {
            Box::pin(async { panic!("test future is not polled") })
        }
    }

    struct Provider {
        properties: ProviderProperties,
    }

    impl AsyncFileSystemSpi for Provider {
        fn properties(&self) -> ProviderProperties {
            self.properties.clone()
        }

        fn stat<'a>(&'a self, _: StatRequest<'a>) -> SpiFuture<'a, FsResult<StatResponse>> {
            Box::pin(async {
                Ok(StatResponse::new(
                    Path::parse("/tmp/recording").expect("valid path"),
                    crate::metadata::FileMetadata::new(FileKind::Directory),
                ))
            })
        }
    }

    fn filesystem() -> AsyncFileSystem {
        let properties = ProviderProperties::new(
            FileSystemInfo::new(
                FileSystemId::new("async-temp-directory-test").expect("valid id"),
                "test",
                PathSemantics::Hierarchical,
            ),
            crate::spi::ProviderOperations::new().with(crate::spi::ProviderOperation::CreateTempDirectory),
            FileSystemCapabilities::new().with_guaranteed(FileSystemCapability::TempDirectory),
            FileSystemLimits::unknown(),
            PathConstraints::absolute(),
            SymlinkPolicy::Reject,
        )
        .expect("valid properties");
        AsyncFileSystem::from_spi(Provider { properties }).expect("valid filesystem")
    }

    #[test]
    fn forwarding_methods_are_executed_at_runtime() {
        let file_system = filesystem();
        let mut directory = AsyncTempDirectory::new(
            file_system,
            Path::parse("/tmp/recording").expect("valid path"),
            Box::new(Session),
        );
        let component = PathComponent::parse("child").expect("valid component");
        let relative = RelativePath::parse("nested/item").expect("valid relative path");

        assert_eq!(directory.path().as_str(), "/tmp/recording");
        assert_eq!(directory.state(), TempResourceState::Owned);
        assert_eq!(directory.child(&component).as_str(), "/tmp/recording/child");
        assert_eq!(directory.descendant(&relative).as_str(), "/tmp/recording/nested/item");
        drop(directory.cleanup());
        drop(directory.keep());
        drop(directory.persist(&Path::parse("/target").expect("valid path"), PersistOptions::default()));
    }
}
