# Qubit FS

Qubit FS 是一个 Rust 抽象文件系统层，用于以统一接口访问本地文件系统、WebDAV、
FTP、OSS、HDFS 以及后续扩展的存储后端。

根 crate 只定义开放契约，不定义封闭的 `FsKind` 枚举；具体后端通过 `qubit-spi`
注册 provider。

## 核心概念

- `FileSystem`：后端无关的文件系统操作接口。
- `FsPath`：provider-local 路径值对象。
- `FsUri`：用于 provider 选择的完整 URI。
- `FileResource`：解析后的资源，内部绑定文件系统和本地路径。
- `FileSystems`：进程级 singleton 门面。
- `FileSystemRegistry`：测试或嵌入式场景可使用的显式隔离 registry。

## 示例

```rust
use qubit_fs::{FileSystems, FsResult};

fn read_report() -> FsResult<Vec<u8>> {
    // provider 注册通常在应用启动阶段完成：
    // FileSystems::register(LocalFileSystemProvider::new())?;

    let resource = FileSystems::resource("file:///var/data/report.csv")?;
    resource.read_all()
}
```

## 文档

- [User guide](doc/user_guide.md)
- [用户指南](doc/user_guide.zh_CN.md)
- [文件系统抽象层设计](doc/file_system_design.zh_CN.md)
