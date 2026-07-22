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
- `FileSystemExt` 与 `AsyncFileSystemExt` 提供整体读取 helper，并要求调用者显式
  提供字节预算；
- directory stream helper 在把枚举收集进内存前要求调用者显式提供最大条目数；
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

`stat` 是文件系统必备操作，不是可选 capability。因此
`FileSystemCapabilities` 只包含可选保证；`FileSystemProperties::limits()`
则返回 provider 稳定的 `FileSystemLimits` 快照。每项限制使用
`FileSystemLimit` 区分 `Unknown`、`NotApplicable`、`Unbounded` 和包含式上限
`Maximum(n)`。provider 必须在直接操作中执行自己声明的有限上限；当请求大小或
规范路径已经可知时，绑定资源与整体读取 helper 会提前检查这些限制。

## 有界聚合

整体读取和完整枚举 helper 不会隐式选择内存上限。调用者必须为每次操作提供预算：

```rust,ignore
let bytes = resource.read_all(8 * 1024 * 1024)?;
let entries = resource
    .list(ListOptions::default())?
    .collect_entries(10_000)?;
```

结果大小恰好等于预算时成功；如果最小探测确认仍有额外字节或条目，则返回
`FsErrorKind::ResourceLimitExceeded`。provider 存储容量或账户配额不足仍返回
`FsErrorKind::QuotaExceeded`。

## 原生路径文本编码

`FsPath`、`FsName` 与 `RelativeFsPath` 存储规范化 UTF-8 路径文本。它是 provider
原生路径字符串的无损表示，并不意味着所有 provider 都原生使用 UTF-8；更不会进行
lossy display conversion。普通 Unicode 保持可读；字面的 `%`、控制字符和 opaque byte
使用大写 `%XX` 转义。

| 原生值或字节 | 规范路径文本 |
| --- | --- |
| `report-中文.txt` | `report-中文.txt` |
| `100%` | `100%25` |
| `66 6F 80 6F` | `fo%80o` |
| `line<LF>break` | `line%0Abreak` |
| Windows 未配对代理项 `D800` | `%ED%A0%80` |

`NativePathCodec` 只转换这个字符串表示，不负责拆分 component、解释 root 或
separator、规范化 dot segment、解析 URI 语法或执行 I/O。本地 `OsStr` 使用
`OsStrPathCodec`；保证严格 UTF-8 byte 的协议使用 `Utf8PathCodec`；允许 opaque
path byte 的兼容 SFTP 或 NFS server 使用 `EscapedBytePathCodec`。三个 codec 都是
只依赖标准库的零大小类型。

路径文本必须使用唯一的规范写法：裸 `%`、格式错误的 escape、小写 hex 和对普通
Unicode 的过度转义都会被拒绝。这使每个原生路径只有一个文本身份，因而可以安全地
作为 `Eq`/`Hash` 的 registry 或 cache key。URI percent encoding 是另一层：规范原生
路径片段 `%25` 在作为 URI path component 编码时会变成 `%2525`。

## 文档

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [文件系统架构设计](doc/file_system_design.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)

## 测试

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行项目 CI 检查
./ci-check.sh

# 检查代码覆盖率
./coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./align-ci.sh` 格式化代码，运行 `./ci-check.sh` 对齐 CI
要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-fs](https://github.com/qubit-ltd/rs-fs)
