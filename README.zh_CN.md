# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

Qubit FS 是一个 Rust 抽象文件系统层，用于以统一接口访问本地文件系统、WebDAV、
FTP、OSS、HDFS 以及后续扩展的存储后端。

根 crate 只定义开放契约，不定义封闭的 `FsKind` 枚举；具体后端通过 `qubit-spi`
注册 provider。

## Provider 模型

- 第三方 provider 实现 `ProviderDefinition<FileSystemSpec>`，由 provider
  自身同时提供创建行为和 descriptor。
- `FileSystemRegistry` 可在运行时注册；它的 clone 共享注册结果和默认选择更新。
- 下游代码可调用 `resolve(&ProviderSelection)` 或 `resolve()` 获得
  `ResolvingServiceProvider<FileSystemSpec>`，再独立提供 `FileSystemConfig`
  创建文件系统。
- `fs(&FsUri)` 是领域便捷接口：它按 URI scheme 选择 provider、创建文件系统，
  并把选择错误和创建错误分别映射为 `FsError`，同时保留原始 source。

## 示例

```rust
use qubit_fs::{FileSystemRegistry, FsResult, FsUri};

fn read_report(registry: &FileSystemRegistry) -> FsResult<Vec<u8>> {
    let uri = FsUri::parse("file:///var/data/report.csv")?;
    let resource = registry.resource(&uri)?;
    resource.read_all()
}

fn configure() -> FsResult<FileSystemRegistry> {
    let registry = FileSystemRegistry::default();
    // 后端 crate 可在应用启动时注册自描述 provider：
    // qubit_fs_local::register_provider(&registry)?;
    Ok(registry)
}
```

## 安装

```toml
[dependencies]
qubit-fs = "0.2"
```

## 文档

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [文件系统抽象层设计](doc/file_system_design.zh_CN.md)

## 测试

```bash
# 使用默认的空 feature 集测试核心 API
cargo test --no-default-features

# 测试核心 API 和正则校验
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
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-fs](https://github.com/qubit-ltd/rs-fs)
