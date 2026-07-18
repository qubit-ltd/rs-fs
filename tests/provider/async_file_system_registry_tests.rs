// =============================================================================
//    Copyright (c) 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================

use std::error::Error;
use std::future::Future;
use std::io::Result as IoResult;
use std::pin::Pin;
use std::sync::{
    Arc,
    Mutex,
};
use std::task::{
    Context,
    Poll,
    Waker,
};

use qubit_fs::{
    AsyncFileReader,
    AsyncFileSystem,
    AsyncFileSystemProvider,
    AsyncFileSystemRegistry,
    CredentialRef,
    FileKind,
    FileLocation,
    FileMetadata,
    FileSystemCapabilities,
    FileSystemConfig,
    FileSystemId,
    FileSystemInfo,
    FileSystemProperties,
    FileSystemResolution,
    FsError,
    FsErrorKind,
    FsFuture,
    FsOperation,
    FsPath,
    FsUri,
    OpenedFileInfo,
    PathSemantics,
    ReadOptions,
};
use qubit_io::AsyncInput;
use qubit_metadata::Metadata;
use qubit_spi::{
    FallbackPolicy,
    ProviderDescriptor,
    ProviderId,
    ProviderSelection,
};

#[derive(Debug)]
struct AsyncOnlyFs {
    info: FileSystemInfo,
}

impl AsyncOnlyFs {
    fn new(id: &str) -> Self {
        Self {
            info: FileSystemInfo::new(
                FileSystemId::new(id).expect("filesystem id should parse"),
                ProviderId::new("async-capture")
                    .expect("provider id should parse"),
                PathSemantics::Hierarchical,
            ),
        }
    }
}

impl FileSystemProperties for AsyncOnlyFs {
    fn info(&self) -> &FileSystemInfo {
        &self.info
    }

    fn capabilities(&self) -> FileSystemCapabilities {
        FileSystemCapabilities::default()
    }
}

impl AsyncFileSystem for AsyncOnlyFs {
    fn stat_async<'a>(
        &'a self,
        _path: &'a FsPath,
    ) -> FsFuture<'a, FileMetadata> {
        Box::pin(async { Ok(FileMetadata::new(FileKind::File)) })
    }

    fn open_reader_async<'a>(
        &'a self,
        path: &'a FsPath,
        _options: ReadOptions,
    ) -> FsFuture<'a, AsyncFileReader> {
        let location = FileLocation::new(self.info.id().clone(), path.clone());
        Box::pin(async move {
            Ok(AsyncFileReader::new(
                EmptyAsyncInput,
                OpenedFileInfo::new(location),
            ))
        })
    }
}

struct EmptyAsyncInput;

impl AsyncInput for EmptyAsyncInput {
    type Item = u8;

    unsafe fn poll_read_unchecked(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        _output: &mut [u8],
        _index: usize,
        _count: usize,
    ) -> Poll<IoResult<usize>> {
        Poll::Ready(Ok(0))
    }
}

struct CapturingAsyncProvider {
    descriptor: ProviderDescriptor,
    captured: Arc<Mutex<Option<FileSystemConfig>>>,
    path: &'static str,
}

impl AsyncFileSystemProvider for CapturingAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }

    fn create_configured_async<'a>(
        &'a self,
        config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        *self.captured.lock().expect("lock should succeed") =
            Some(config.clone());
        let fs: Arc<dyn AsyncFileSystem> =
            Arc::new(AsyncOnlyFs::new("async-only"));
        let path = FsPath::parse_literal(self.path)
            .expect("provider path should parse");
        let uri = config.uri().clone();
        Box::pin(async move { Ok(FileSystemResolution::new(fs, path, uri)) })
    }
}

struct UnavailableAsyncProvider {
    descriptor: ProviderDescriptor,
}

struct ErrorAsyncProvider {
    descriptor: ProviderDescriptor,
    kind: FsErrorKind,
}

impl AsyncFileSystemProvider for ErrorAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }

    fn create_configured_async<'a>(
        &'a self,
        _config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        let kind = self.kind;
        Box::pin(async move {
            Err(FsError::new(
                kind,
                FsOperation::Provider,
                "provider creation failed",
            ))
        })
    }
}

impl AsyncFileSystemProvider for UnavailableAsyncProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        self.descriptor.clone()
    }

    fn create_configured_async<'a>(
        &'a self,
        _config: &'a FileSystemConfig,
    ) -> FsFuture<'a, FileSystemResolution<dyn AsyncFileSystem>> {
        Box::pin(async {
            Err(FsError::new(
                FsErrorKind::ProviderUnavailable,
                FsOperation::Provider,
                "provider is unavailable",
            ))
        })
    }
}

fn ready<F>(future: F) -> F::Output
where
    F: Future,
{
    let mut context = Context::from_waker(Waker::noop());
    let mut future = std::pin::pin!(future);
    match future.as_mut().poll(&mut context) {
        Poll::Ready(output) => output,
        Poll::Pending => panic!("test future should be immediately ready"),
    }
}

#[test]
fn async_registry_accepts_async_only_provider_and_passes_complete_config() {
    let captured = Arc::new(Mutex::new(None));
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: captured.clone(),
            path: "provider-decoded/%2F",
        })
        .expect("async provider should register");
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///raw%2Fkey").expect("URI should parse"),
    )
    .with_selection(
        ProviderSelection::named("async-capture")
            .expect("selection should parse"),
    )
    .with_options(Metadata::new().with("region", "test-1".to_owned()))
    .expect("options should be valid")
    .with_credentials(CredentialRef::Profile("integration".to_owned()));

    let resource = ready(registry.resource_async(&config))
        .expect("complete config should resolve asynchronously");

    assert_eq!("provider-decoded/%2F", resource.path().as_str());
    assert_eq!(
        Some(config),
        captured.lock().expect("lock should succeed").clone()
    );
    let reader = ready(resource.open_reader_async(ReadOptions::default()))
        .expect("reader should open");
    assert_eq!(
        resource.location().uri(),
        reader.info().location().uri(),
        "registry canonical identity should reach asynchronous handles",
    );
}

#[test]
fn async_registry_applies_absence_fallback_after_awaiting_creation() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(UnavailableAsyncProvider {
            descriptor: descriptor("offline"),
        })
        .expect("unavailable provider should register");
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "fallback-result",
        })
        .expect("fallback provider should register");
    let selection = ProviderSelection::chain(["offline", "async-capture"])
        .expect("selection should parse")
        .with_fallback_policy(FallbackPolicy::OnAbsence);
    let config = FileSystemConfig::new(
        FsUri::parse("async-capture:///resource").expect("URI should parse"),
    )
    .with_selection(selection);

    let resolution = ready(registry.resolve_config_async(&config))
        .expect("absence fallback should reach the second provider");

    assert_eq!("fallback-result", resolution.path().as_str());
}

#[test]
fn async_registry_rejects_conflicting_provider_selectors() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "first",
        })
        .expect("first provider should register");

    let error = registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "second",
        })
        .expect_err("duplicate selector should fail atomically");

    assert_eq!(FsErrorKind::Conflict, error.kind());
    assert_eq!(vec!["async-capture"], registry.provider_ids());
}

#[test]
fn async_registry_exposes_default_and_uri_convenience_paths() {
    let registry = AsyncFileSystemRegistry::default();
    assert!(registry.default_selection().selectors().is_empty());
    let provider: Arc<dyn AsyncFileSystemProvider> =
        Arc::new(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "resolved",
        });
    registry
        .register_shared(provider)
        .expect("shared provider should register");
    let selection = ProviderSelection::named("async-capture")
        .expect("selection should parse");
    registry.set_default_selection(selection.clone());
    assert_eq!(
        selection.selectors(),
        registry.default_selection().selectors()
    );

    let uri = FsUri::parse("async-capture:///resource").unwrap();
    let config = FileSystemConfig::new(uri.clone());
    assert_eq!(
        "resolved",
        ready(registry.resolve_async(&config))
            .unwrap()
            .path()
            .as_str(),
    );
    assert_eq!(
        "resolved",
        ready(registry.resolve_selected_async(&selection, &config))
            .unwrap()
            .path()
            .as_str(),
    );
    assert_eq!(
        "async-only",
        ready(registry.file_system_async(&config))
            .unwrap()
            .info()
            .id()
            .as_str(),
    );
    assert_eq!(
        "async-only",
        ready(registry.file_system_uri_async(&uri))
            .unwrap()
            .info()
            .id()
            .as_str(),
    );
    assert_eq!(
        "resolved",
        ready(registry.resource_uri_async(&uri))
            .unwrap()
            .path()
            .as_str(),
    );
}

#[test]
fn empty_async_registry_reports_provider_unavailable_from_every_entry_point() {
    let registry = AsyncFileSystemRegistry::default();
    let uri = FsUri::parse("missing:///resource").unwrap();
    let config = FileSystemConfig::new(uri.clone());
    let selection =
        ProviderSelection::named("missing").expect("selection should parse");

    let errors = [
        ready(registry.resolve_async(&config)).unwrap_err(),
        ready(registry.resolve_config_async(&config)).unwrap_err(),
        ready(registry.resolve_selected_async(&selection, &config))
            .unwrap_err(),
        ready(registry.file_system_async(&config))
            .err()
            .expect("filesystem creation should fail"),
        ready(registry.resource_async(&config)).unwrap_err(),
        ready(registry.file_system_uri_async(&uri))
            .err()
            .expect("URI filesystem creation should fail"),
        ready(registry.resource_uri_async(&uri)).unwrap_err(),
    ];
    for error in errors {
        assert_eq!(FsErrorKind::ProviderUnavailable, error.kind());
    }

    let invalid_selector_config = FileSystemConfig::new(
        FsUri::parse("missing-:///resource").expect("URI scheme should parse"),
    );
    assert_eq!(
        FsErrorKind::ProviderUnavailable,
        ready(registry.resolve_config_async(&invalid_selector_config))
            .unwrap_err()
            .kind(),
    );
}

#[test]
fn async_registry_applies_each_creation_fallback_policy() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(ErrorAsyncProvider {
            descriptor: descriptor("broken"),
            kind: FsErrorKind::Other,
        })
        .unwrap();
    registry
        .register(ErrorAsyncProvider {
            descriptor: descriptor("unsupported"),
            kind: FsErrorKind::UnsupportedCapability,
        })
        .unwrap();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("async-capture"),
            captured: Arc::new(Mutex::new(None)),
            path: "fallback",
        })
        .unwrap();
    let config = FileSystemConfig::new(
        FsUri::parse("async-capture:///resource").expect("URI should parse"),
    );

    let never = ProviderSelection::chain(["broken", "async-capture"])
        .unwrap()
        .with_fallback_policy(FallbackPolicy::Never);
    let error = ready(registry.resolve_selected_async(&never, &config))
        .expect_err("never policy should stop at the first error");
    assert_eq!(FsErrorKind::Other, error.kind());
    assert_eq!(Some("broken"), error.provider().map(ProviderId::as_str));

    let absence = ProviderSelection::chain(["broken", "async-capture"])
        .unwrap()
        .with_fallback_policy(FallbackPolicy::OnAbsence);
    assert_eq!(
        FsErrorKind::Other,
        ready(registry.resolve_selected_async(&absence, &config))
            .unwrap_err()
            .kind(),
    );

    let unsupported =
        ProviderSelection::chain(["unsupported", "async-capture"])
            .unwrap()
            .with_fallback_policy(FallbackPolicy::OnAbsence);
    assert_eq!(
        "fallback",
        ready(registry.resolve_selected_async(&unsupported, &config))
            .unwrap()
            .path()
            .as_str(),
    );

    let any = ProviderSelection::chain(["broken", "async-capture"])
        .unwrap()
        .with_fallback_policy(FallbackPolicy::OnAnyError);
    assert_eq!(
        "fallback",
        ready(registry.resolve_selected_async(&any, &config))
            .unwrap()
            .path()
            .as_str(),
    );
}

#[test]
fn async_registry_retains_ordered_failures_when_fallback_is_exhausted() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(UnavailableAsyncProvider {
            descriptor: descriptor("first-offline"),
        })
        .unwrap();
    registry
        .register(ErrorAsyncProvider {
            descriptor: descriptor("second-unsupported"),
            kind: FsErrorKind::UnsupportedOperation,
        })
        .unwrap();
    let selection =
        ProviderSelection::chain(["first-offline", "second-unsupported"])
            .unwrap()
            .with_fallback_policy(FallbackPolicy::OnAbsence);
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    );

    let error = ready(registry.resolve_selected_async(&selection, &config))
        .expect_err("every admitted provider should fail");

    assert_eq!(FsErrorKind::ProviderUnavailable, error.kind());
    assert_eq!(None, error.provider());
    let source = error.source().expect("aggregate source should be retained");
    let debug = format!("{source:?}");
    assert!(debug.contains("attempt_count: 2"));
    assert!(!debug.contains("first-offline"));
    let diagnostics = source.to_string();
    let first = diagnostics
        .find("first-offline")
        .expect("first failure should be present");
    let second = diagnostics
        .find("second-unsupported")
        .expect("second failure should be present");
    assert!(
        first < second,
        "provider failures must preserve attempt order"
    );
}

#[test]
fn async_registry_aggregates_failures_before_policy_stops_fallback() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(UnavailableAsyncProvider {
            descriptor: descriptor("first-offline"),
        })
        .unwrap();
    registry
        .register(ErrorAsyncProvider {
            descriptor: descriptor("second-broken"),
            kind: FsErrorKind::Other,
        })
        .unwrap();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("unreached"),
            captured: Arc::new(Mutex::new(None)),
            path: "unreached",
        })
        .unwrap();
    let selection = ProviderSelection::chain([
        "first-offline",
        "second-broken",
        "unreached",
    ])
    .unwrap()
    .with_fallback_policy(FallbackPolicy::OnAbsence);
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    );

    let error = ready(registry.resolve_selected_async(&selection, &config))
        .expect_err("non-absence failure should stop fallback");

    assert_eq!(FsErrorKind::Other, error.kind());
    let diagnostics = error
        .source()
        .expect("aggregate source should be retained")
        .to_string();
    assert!(diagnostics.contains("fallback stopped"));
    assert!(diagnostics.contains("first-offline"));
    assert!(diagnostics.contains("second-broken"));
    assert!(!diagnostics.contains("unreached"));
}

#[test]
fn async_registry_automatic_priority_aliases_and_deduplication_are_stable() {
    let registry = AsyncFileSystemRegistry::default();
    registry
        .register(CapturingAsyncProvider {
            descriptor: descriptor("low").with_priority(1),
            captured: Arc::new(Mutex::new(None)),
            path: "low",
        })
        .unwrap();
    let high_descriptor = descriptor("high")
        .with_aliases(["fast"])
        .expect("alias should parse")
        .with_priority(10);
    registry
        .register(CapturingAsyncProvider {
            descriptor: high_descriptor,
            captured: Arc::new(Mutex::new(None)),
            path: "high",
        })
        .unwrap();
    let config = FileSystemConfig::new(
        FsUri::parse("unrelated:///resource").expect("URI should parse"),
    );

    assert_eq!(
        "high",
        ready(registry.resolve_async(&config))
            .unwrap()
            .path()
            .as_str(),
    );
    let deduplicated = ProviderSelection::chain(["fast", "high"]).unwrap();
    assert_eq!(
        "high",
        ready(registry.resolve_selected_async(&deduplicated, &config))
            .unwrap()
            .path()
            .as_str(),
    );

    let conflicting = descriptor("other")
        .with_aliases(["fast"])
        .expect("alias should parse");
    assert_eq!(
        FsErrorKind::Conflict,
        registry
            .register(CapturingAsyncProvider {
                descriptor: conflicting,
                captured: Arc::new(Mutex::new(None)),
                path: "other",
            })
            .unwrap_err()
            .kind(),
    );
    assert_eq!(vec!["low", "high"], registry.provider_ids());
}

fn descriptor(id: &str) -> ProviderDescriptor {
    ProviderDescriptor::new(
        ProviderId::new(id).expect("provider id should parse"),
    )
}
