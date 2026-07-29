# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

Qubit FS 是 provider-neutral 的文件系统抽象。应用使用具体的 `FileSystem` 和
`AsyncFileSystem` facade；provider 只在 `qubit_fs::spi` 命名空间实现契约。运行时
provider 发现和配置属于
[`qubit-fs-registry`](https://crates.io/crates/qubit-fs-registry)。

```toml
[dependencies]
qubit-fs = "0.2"
```

## 应用 API

由 provider SPI 创建具体 facade，再用 `Path` 表示逻辑资源路径。

```rust,ignore
use qubit_fs::{FileSystem, Path, ReadOptions};
use qubit_fs::spi::FileSystemSpi;

fn read_metadata<S: FileSystemSpi + 'static>(provider: S) -> qubit_fs::FsResult<()> {
    let filesystem = FileSystem::from_spi(provider)?;
    let path = Path::parse("/reports/2026/summary.csv")?;
    let _metadata = filesystem.stat(&path)?;
    let _reader = filesystem.open_reader(&path, ReadOptions::default())?;
    Ok(())
}
```

copy 和 rename 会返回带恢复状态的 typed failure。writer 与临时资源句柄在可恢复失败后
仍提供显式的 `abort`、`cleanup`、`keep` 或 `persist` 生命周期操作。
`AsyncFileSystem::begin_copy` 返回 `AsyncCopyOperation`；应用应通过自己的运行时轮询
其执行 future，并在取消后检查操作状态。

`Uri` 会拒绝含凭据的字段。`ConnectionUri` 可为连接用途携带凭据，但其 `Display` 与
`Debug` 输出会遮蔽凭据；应用仍不得暴露原始值。即使应用在进程级默认脱敏策略中安装
allow 规则，`UserMetadata` 仍会拒绝 credential-like key。`FileSystemProperties`
是不触发 I/O 的不可变快照，包含
capability、limit 和逻辑路径约束。

## 文档

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [文件系统架构设计](doc/file_system_design.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)

## 测试

```bash
# 运行默认功能集测试
cargo test

# 运行所有声明功能测试
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
Pull Request 前运行 `./align-ci.sh` 格式化代码，运行 `./ci-check.sh` 对齐 CI 要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-fs](https://github.com/qubit-ltd/rs-fs)
