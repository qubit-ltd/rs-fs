# Qubit FS

[![Rust CI](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml/badge.svg)](https://github.com/qubit-ltd/rs-fs/actions/workflows/ci.yml)
[![Coverage](https://img.shields.io/endpoint?url=https://qubit-ltd.github.io/rs-fs/coverage-badge.json)](https://qubit-ltd.github.io/rs-fs/coverage/)
[![Crates.io](https://img.shields.io/crates/v/qubit-fs.svg?color=blue)](https://crates.io/crates/qubit-fs)
[![Rust](https://img.shields.io/badge/rust-1.94+-blue.svg?logo=rust)](https://www.rust-lang.org)
[![License](https://img.shields.io/badge/license-Apache%202.0-blue.svg)](LICENSE)
[![English Document](https://img.shields.io/badge/Document-English-blue.svg)](README.md)

`qubit-fs` 是 Rust 1.94 及以上版本可用的、与具体 provider 无关的文件系统抽象，
同时提供同步和异步 API。它向应用提供具体门面 `FileSystem` 与
`AsyncFileSystem`，但不会替应用选择存储后端或异步运行时。

## 安装

```toml
[dependencies]
qubit-fs = "0.7"
```

同步 API 默认启用。需要异步门面时必须显式开启 async feature：

```toml
qubit-fs = { version = "0.7", features = ["async"] }
```

## 快速开始

报告任务通过 `FileSystem` 完成读写，由初始化代码选择存储位置。下面的完整示例创建独立的
本地临时目录，写入报告，再以 1 KiB 上限读取，输出 `report ready`。退出时清理临时目录。

```toml
[dependencies]
qubit-fs = "0.7"
qubit-fs-local = "0.8"
tempfile = "3"
```

<!-- example: quick-start -->
```rust
use qubit_fs::Path;
use qubit_fs::directory::ListScope;
use qubit_fs::read::ReadOptions;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let policy = LocalResourcePolicy::standard();
    let filesystem = LocalFileSystems::rooted(directory.path(), policy)?;
    let path = Path::parse("/report.txt")?;
    filesystem.write_all(&path, b"report ready", WriteOptions::default())?;
    let bytes = filesystem.read_all(&path, ReadOptions::default(), 1024)?;
    assert_eq!(b"report ready", bytes.as_slice());
    let scope = ListScope::Path(Path::root());
    let mut entries = filesystem.list(&scope, Default::default())?;
    assert_eq!(entries.next_entry()?.expect("published report").path, path);
    assert!(entries.next_entry()?.is_none());
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}
```

如果报告需要分多次写入，可先在同一逻辑父目录创建临时文件，通过返回的路径写入，全部生成
完成后再调用 `persist`。这样最终报告名称不会提前暴露未完成内容。发布失败时，应结合保留的
发布事实、源资格和 `publication_target()`，在重试、cleanup 与只读核查之间作出选择。

## 为什么需要这个项目

业务代码往往需要在本地磁盘、对象存储和托管文件系统之间复用同一套读写、复制和
列举流程，但各 provider SDK 的路径模型、错误形态和部分失败后的恢复方式并不一致。
如果把 provider 细节散落到业务逻辑里，重试、cleanup 和取消后的状态就很难判断。

`qubit-fs` 让应用只依赖具体的 `FileSystem` 与 `AsyncFileSystem` 门面，provider 在
`qubit_fs::spi` 下实现扩展契约；发现、配置和凭据接入由 `qubit-fs-registry` 负责，
核心 crate 不内置后端，也不绑定异步运行时。

## 提供什么，以及不提供什么

稳定的应用侧能力包括公共门面 API、类型化路径与 URI、明确的列举范围、有界读取，
以及在写入、复制或取消未正常完成时保留发布事实的恢复对象。完整工作流、错误表和
运行限制见用户手册；provider 集成见 provider 指南。

门面对下列语义做了显式约定：

- `Path` 是一个已配置 filesystem 内的逻辑名称。`Uri` 是不含 secret 的规范
  地址；`ConnectionUri` 是配置入口，可以接受凭据，但在 `Display` 和 `Debug` 中会
  脱敏。
- 默认 URI 解析使用固定的标准脱敏策略；如果应用还有自定义敏感 query 名称，应向
  `Uri::parse_with_policy` 或 `ConnectionUri::parse_with_policy` 显式传入策略。
- copy、rename、写入和临时资源发布会保留带类型的恢复事实。临时资源持久化分别报告
  目标发布事实与源资格；`publication_target()` 会在调用被拒绝、清理失败和异步取消后
  继续保留先前已确认的目标。重试、清理或核对可见目标前，应同时检查这两类事实。
- `exists` 只有在 `stat` 明确返回 `NotFound` 时才返回 `false`；权限、认证、超时和
  I/O 失败仍会作为错误返回。
- `DirectoryStream` 按条目增量读取。应在有界循环中消费它，不应把目录假定为已加载的
  集合。
- 列举范围由 `ListScope::Path` 或平面命名空间 `ListScope::Namespace` 明确指定；前缀读取
  仅在 `RangeRead` 为 Guaranteed 时自动添加范围。

核心 crate 不含内置后端，也不会替应用选择异步运行时。它不提供跨文件系统 move，
不保证所有 provider 都支持全部能力，也不会在无 provider 规则的情况下把对象键自动
变成层级路径。平台根目录和权限边界由所配置的 provider 决定。

## 延伸阅读

- [English user guide](doc/user_guide.md)
- [中文用户手册](doc/user_guide.zh_CN.md)
- [English provider guide](doc/provider_guide.md)
- [中文 provider 指南](doc/provider_guide.zh_CN.md)
- [English architecture](doc/file_system_design.md)
- [中文架构设计](doc/file_system_design.zh_CN.md)
- [docs.rs API 文档](https://docs.rs/qubit-fs)
- [English README](README.md)
- [仓库地址](https://github.com/qubit-ltd/rs-fs)

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
Pull Request 前运行 `./align-ci.sh` 格式化代码，运行 `./ci-check.sh` 对齐 CI 要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-fs](https://github.com/qubit-ltd/rs-fs)
