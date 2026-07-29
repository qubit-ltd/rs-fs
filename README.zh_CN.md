# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

`qubit-fs` 0.2.0 是 Rust 1.94 及以上版本可用的 provider-neutral 文件系统抽象，
同时提供同步和异步 API。它向应用提供具体门面 `FileSystem` 与
`AsyncFileSystem`，但不会替应用选择存储后端或异步运行时。

核心 crate 不含内置后端。provider 在 `qubit_fs::spi` 下实现扩展契约；provider 的
发现、配置和凭据处理由 `qubit-fs-registry` 负责。因此应用代码只依赖公共门面，
而 provider 的选择留在核心 crate 之外。

```toml
[dependencies]
qubit-fs = "0.2"
```

## 门面明确表达的语义

- `Path` 是一个已配置 filesystem 内的逻辑名称。`Uri` 是不含 secret 的 canonical
  地址；`ConnectionUri` 是配置入口，可以接受凭据，但在 `Display` 和 `Debug` 中会
  脱敏。
- copy、rename、写入和临时资源发布会保留带类型的恢复事实。重试、清理或核对已经
  可见的目标前，应先检查对应的 failure state。
- `exists` 只有在 `stat` 明确返回 `NotFound` 时才返回 `false`；权限、认证、超时和
  I/O 失败仍会作为错误返回。
- `DirectoryStream` 按条目增量读取。应在有界循环中消费它，不应把目录假定为已加载的
  集合。

## 从这里开始

- [English user guide](doc/user_guide.md)
- [中文用户指南](doc/user_guide.zh_CN.md)
- [中文架构设计](doc/file_system_design.zh_CN.md)
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
Pull Request 前运行 `./align-ci.sh`格式化代码，运行`./ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-fs](https://github.com/qubit-ltd/rs-fs)
