# Qubit FS

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

`Uri` 和 `ConnectionUri` 保留 URI 语法，同时拒绝含凭据的字段；即使应用在进程级默认
脱敏策略中安装 allow 规则，这些凭据边界也不会放宽。`UserMetadata` 同样
拒绝 credential-like key。`FileSystemProperties` 是不触发 I/O 的不可变快照，包含
capability、limit 和逻辑路径约束。

## 文档

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [文件系统架构设计](doc/file_system_design.zh_CN.md)
- [API 文档](https://docs.rs/qubit-fs)

## 测试

```bash
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
```

本项目使用 Apache License 2.0，详见 [LICENSE](LICENSE)。
