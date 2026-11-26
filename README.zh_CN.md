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

## 快速开始

2026-11-24 查询 crates.io 公开索引时，`qubit-fs` 最高版本为 0.3.3，尚无
0.4 发布版本。当前请在 `rs-fs` 同级创建独立 Cargo 项目（例如 `report-app`），
将下方标记的依赖配置放入其 `Cargo.toml`，并把[直接调用兼容示例](#直接调用兼容示例)
中的完整程序复制到 `report-app/src/main.rs`。运行 `cargo run --manifest-path ../report-app/Cargo.toml`，
预期输出 `report ready`。源码路径会从同级 checkout 加载核心库和本机 adapter。

<!-- dependencies: quick-start -->
```toml
[dependencies]
qubit-fs = { version = "0.4", path = "../rs-fs" }
qubit-fs-local = { path = "../rs-fs-local" }
tempfile = "3"
```

<!-- dependencies: quick-start-async -->
```toml
[dependencies]
qubit-fs = { version = "0.4", path = "../rs-fs", features = ["async"] }
```

<!-- release-status: quick-start -->
```toml
[release]
core_status = "not-published"
checked = "2026-11-24"
```

默认特性集合为空：提供同步 API，且不会启动线程。可选的 `async` feature
提供异步 API，但不选择或启动执行器。0.4 引入 owned provider 操作和显式执行绑定；请阅读
[托管操作迁移说明](doc/user_guide.zh_CN.md#托管操作与-03-迁移)及
[provider 指南](doc/provider_guide.zh_CN.md#托管-spi-迁移)。

## 直接调用兼容示例

报告任务通过 `FileSystem` 完成读写，由初始化代码选择存储位置。下面的完整示例创建独立的
本地临时目录，写入报告，再以 1 KiB 上限读取，输出 `report ready`。退出时清理临时目录。此直接调用示例展示
非托管兼容 API。文中引用的 `qubit-fs-local` 版本未被声明为实现了 0.4 SPI；集成使用时应选择为 0.4 构建的 provider。

源码 fixture 使用同级 `rs-fs-local` 0.12.0 候选源码；其 manifest 依赖 `qubit-fs` 0.4 和
`qubit-local-files` 0.6.0。上面的独立项目使用相同源码布局。
`qubit-operation` 使用 crates.io 上的 `^0.3.1`，不需要 `rs-operation` checkout。

<!-- example: quick-start -->
```rust
// =============================================================================
//    Copyright (c) 2025 - 2026 Haixing Hu.
//
//    SPDX-License-Identifier: Apache-2.0
//
//    Licensed under the Apache License, Version 2.0.
// =============================================================================
//! Demonstrates basic operations on a rooted local filesystem.

use qubit_fs::Path;
use qubit_fs::copy::CopyExecutionRoute;
use qubit_fs::copy::CopyOptions;
use qubit_fs::directory::ListScope;
use qubit_fs::read::PrefixReadTermination;
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
    let prefix = filesystem.read_prefix(&path, ReadOptions::default(), 6)?;
    assert_eq!(prefix.bytes(), b"report");
    assert_eq!(prefix.termination(), PrefixReadTermination::LimitReached);
    let target = Path::parse("/report-copy.txt")?;
    let assessment = filesystem.assess_copy(&path, &target, &CopyOptions::default())?;
    assert_eq!(assessment.route(), CopyExecutionRoute::ProviderThenStream);
    assert_eq!(assessment.fallback_rejection(), None);
    let scope = ListScope::Path(Path::root());
    let mut entries = filesystem.list(&scope, Default::default())?;
    assert_eq!(entries.next_entry()?.expect("published report").path, path);
    assert!(entries.next_entry()?.is_none());
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}
```

`assess_copy` 会在不调用 provider I/O 的情况下完成静态验证和路由评估：依次检查路径、选项与能力、同路径；零条目预算返回 `ResourceLimitExceeded`，并且不会检查动态执行期限。

`WriteOptions::default()` 会替换已有目标；使用 `CreateNew` 可拒绝覆盖。配置方式见
[中文用户手册](doc/user_guide.zh_CN.md)。

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
- 需要反复从不同位置采样时，可用 `open_random_access_reader` 固定一次打开的对象，再按偏移
  将数据读入调用方缓冲区。缓存长度只限定本次会话在打开时观察到的可访问范围，不是内容快照；
  provider 可以不支持这一可选 operation。回退和 provider 实现规则见用户手册与 provider 指南。

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

统一的文档示例入口会检查双语示例、版本和包内相对链接，再编译、测试并运行 fixture。
在同级 Qubit 源码仓库的 `rs-fs` 根目录中运行：

```bash
bash scripts/check-documentation-examples.sh
```

独立 workflow 使用固定的同级仓库版本运行此入口；本地入口使用当前检出的同级仓库内容。
源码 manifest 需要 `rs-fs-local` 0.12.0 候选源码、`rs-fs-registry`、`rs-fs-testkit` 和
`rs-local-files` 0.6.0 与 `rs-fs` 位于同一父目录。`rs-redact` 发布的 `qubit-redact` 0.10 和
`qubit-operation` 0.3.1 均从 crates.io 解析，不需要对应源码 checkout。

<!-- source-layout -->
```text
parent/
├── rs-fs/
├── rs-fs-local/
├── rs-fs-registry/
├── rs-fs-testkit/
└── rs-local-files/
```

仅克隆 `rs-fs` 或下载 crate 源码包不包含该本地布局。兼容的 0.4 发布版本可用后，
可在独立 Cargo 项目中使用“快速开始”的依赖配置与完整程序验证 registry 安装。当前源码 fixture 运行：

文档 workflow 还会在集成验证前运行入口回归测试；核心 CI 仍检查本 crate 及其文档。

fixture 使用固定的同级 revision 验证仓库源码，不验证 crates.io 发布包的安装；兼容发布版本可用后，
再用独立项目验证 registry 安装。

```bash
# 使用默认 feature 集运行测试
cargo test

# 使用项目声明的全部 feature 运行测试
cargo test --all-features

# 运行项目 CI 检查
./.infra/bin/ci-check.sh

# 检查代码覆盖率
./.infra/bin/coverage.sh
```

## 许可证

Copyright (c) 2025 - 2026. Haixing Hu. All rights reserved.

本项目基于 Apache License 2.0 授权。完整许可证文本请参阅
[LICENSE](LICENSE)。

## 贡献

欢迎贡献。请遵循 Rust API 指南，及时更新公共 API 文档与测试，并在提交
Pull Request 前运行 `./.infra/bin/align-ci.sh`格式化代码，运行`./.infra/bin/ci-check.sh`对齐CI要求。

## 作者

**Haixing Hu** - *Qubit Co. Ltd.*

仓库地址：[https://github.com/qubit-ltd/rs-fs](https://github.com/qubit-ltd/rs-fs)
