# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

Qubit FS 是一个 provider-neutral 的文件系统抽象层，面向本地、远程、云端、云盘、
分布式和虚拟存储后端。

本 crate 定义契约，不内置具体后端：

- `FileSystemProperties` 暴露构造时确定、不会触发 I/O 的信息；
- `FileSystem` 提供同步操作；
- `AsyncFileSystem` 提供运行时无关的异步操作；
- `FileSystemExt` 与 `AsyncFileSystemExt` 提供有界资源的整体读写 helper，不扩张
  provider trait；
- 文件句柄使用 `qubit_io::Input` / `Output` 与
  `AsyncInput` / `AsyncOutput`；
- `FsUri` 定位资源，`FsPath` 表示 provider 解码后、某个已配置文件系统内部的
  路径；
- 类型化的能力、要求、结果和错误保留 POSIX 文件系统、对象存储、云盘和分布式
  文件系统之间真实存在的语义差异；
- 同步与异步 registry 都会把完整 `FileSystemConfig` 传给可插拔 provider。

本 crate 不内置本地或远程 provider。应用在启动时组装所需后端 crate。

## 安装

```toml
[dependencies]
qubit-fs = "0.2"
```

## 同步解析

```rust
use qubit_fs::{
    CredentialRef,
    FileResource,
    FileSystemConfig,
    FileSystemRegistry,
    FsResult,
    FsUri,
};

fn resolve_report(
    registry: &FileSystemRegistry,
) -> FsResult<FileResource> {
    let uri = FsUri::parse(
        "s3://reports/2026/summary.csv?region=us-east-1",
    )?;
    let config = FileSystemConfig::new(uri)
        .with_credentials(CredentialRef::Profile("analytics".into()));
    registry.resource(&config)
}
```

如果 `FileSystemConfig` 没有显式 provider selection，registry 会按 URI scheme
选择 provider。`resource_uri()` 是只使用 URI 的便捷方法。

## 异步解析

```rust
use qubit_fs::{
    AsyncFileResource,
    AsyncFileSystemRegistry,
    FileSystemConfig,
    FsResult,
    FsUri,
};

async fn resolve_report(
    registry: &AsyncFileSystemRegistry,
) -> FsResult<AsyncFileResource> {
    let config = FileSystemConfig::new(
        FsUri::parse("s3://reports/2026/summary.csv")?,
    );
    registry.resource_async(&config).await
}
```

异步文件系统方法统一使用 `_async` 后缀。Open 本身是异步操作，完成后返回已经初始化
好的 `AsyncFileReader` 或 `AsyncFileWriter`。

## 语义保证

`AtomicityRequirement::Required` 是硬契约：provider 必须在产生副作用前拒绝无法
满足的保证，不能静默降级。成功的 write、rename、copy 和临时资源 persist 都会
报告实际达到的 atomicity 与具体 publication method。

Writer 和临时资源句柄会在可恢复失败后保留 provider session。临时资源 persist
还会返回
`PersistFailureState::{NotPublished, PublishedSourceRetained, Indeterminate}`，
使调用方能明确选择重试、清理或对账。

`FsUri` 保留原始编码 path、重复 query 的顺序，以及 `scheme:/path` 与
`scheme:///path` 的区别。URI path 由 provider 解码；必须转义的 literal path 字符
应预先 percent encode。URI 会拒绝 password、token 等 credential 字段；所有可被
Debug 观察的扩展 metadata（config options、filesystem/file metadata 与 operation
diagnostics）统一使用 `NonSensitiveMetadata`，递归检查顶层、string map 与数组内
JSON object 的 credential-like key，且 Debug 只输出 key。普通 scalar value 无法
可靠分类，因此所有 secret 都必须通过 `CredentialRef` 引用。

## 文档

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [文件系统架构设计](doc/file_system_design.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)

## 开发

```bash
cargo test
./align-ci.sh
RS_CI_SKIP_TOOLCHAIN_UPDATE=1 ./ci-check.sh
```

## 许可证

本项目使用 Apache License 2.0，完整文本见 [LICENSE](LICENSE)。

Copyright (c) 2025 - 2026 Haixing Hu.
