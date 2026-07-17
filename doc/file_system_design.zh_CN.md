# Qubit FS 抽象文件系统层设计

## 1. 背景与目标

`rs-fs` 的目标是提供一个类似 Java `FileSystem` 的 Rust 抽象文件系统层，使上层业务可以用统一接口访问多种存储后端：

- 本地文件系统：`file://` / `local`
- FTP / SFTP：`ftp://`、`sftp://`
- WebDAV：`webdav://`、`webdavs://`
- 对象存储：`oss://`、`s3://`、`cos://`
- 分布式文件系统：`hdfs://`
- 未来扩展：内存文件系统、测试文件系统、加密文件系统、缓存文件系统、组合挂载文件系统

设计必须满足以下约束：

- `rs-fs` 根 crate 不枚举所有后端类型，不定义 `FsKind::Local/Oss/Hdfs` 这类固定枚举。
- 新增后端应通过新 crate 实现并注册 provider，不要求修改 `rs-fs` 核心接口。
- 统一路径模型不能直接等同于 `std::path::Path`，但本地后端内部可以继续使用 `Path` / `PathBuf`。
- 统一接口要能表达本地文件、远端对象、目录、前缀、符号链接、能力缺失、条件写入、原子替换等差异。
- MVP 优先复用现有 Qubit crate：`qubit-spi`、`qubit-io`、`qubit-metadata`、`qubit-local-files`，必要时再引入 `qubit-atomic`。

## 2. 仓库与 crate 分层

推荐拆成核心 crate 与后端 crate：

```text
rust-common/
  rs-fs/                 # package: qubit-fs，核心抽象与 registry
  rs-fs-local/           # package: qubit-fs-local，本地实现
  rs-fs-webdev/          # package: qubit-fs-webdev，WebDAV 实现
  rs-fs-oss/             # package: qubit-fs-oss，OSS 实现
  rs-fs-ftp/             # package: qubit-fs-ftp，FTP/SFTP 实现
  rs-fs-hdfs/            # package: qubit-fs-hdfs，HDFS 实现
```

`qubit-fs` 只定义通用契约：

- 路径与 URI：`FsPath`、`FsUri`、`FsAuthority`
- 文件系统 trait：`FileSystem`、`FileReader`、`FileWriter`
- 元信息：`FileMetadata`、`FileKind`、`DirEntry`
- 能力声明：`FileSystemCapabilities`
- 错误模型：`FsError`、`FsErrorKind`
- provider SPI：`FileSystemSpec`、`FileSystemProvider`、`ProviderDefinition`
- 运行时可注册且 clone 共享状态的 registry：`FileSystemRegistry`
- 资源对象：`FileResource`

具体实现放在后端 crate 中：

- `qubit-fs-local` 依赖 `qubit-fs` 和 `qubit-local-files`
- `qubit-fs-webdev` 依赖 `qubit-fs` 和 WebDAV HTTP client，用于实现 WebDAV 协议文件系统
- `qubit-fs-oss` 依赖 `qubit-fs` 和 OSS SDK
- `qubit-fs-hdfs` 依赖 `qubit-fs` 和 HDFS client
- 后端 crate 可以暴露 `register_provider(&FileSystemRegistry)`，也可以暴露自描述 provider 类型给应用显式注册

这样新增 `webdav://`、`s3://`、`azblob://` 之类后端时，只新增一个 crate 和 provider，不需要修改 `qubit-fs` 根 crate。

## 3. 为什么不用 `FsKind` 固定枚举

固定枚举会把“新增后端”变成核心 crate 的破坏性变更：

```rust
pub enum FsKind {
    Local,
    Ftp,
    Oss,
    Hdfs,
}
```

这种设计的问题：

- 每新增一个后端都要改 `qubit-fs`，核心 crate 被具体实现拖着发版。
- 下游 `match FsKind` 的代码会被迫处理新 variant，或者只能退回 `_` 分支，语义不清。
- provider 选择、优先级、别名、运行时可用性都不适合塞进枚举。
- 对外暴露枚举后，第三方后端无法自然接入。

替代方案是使用字符串化 provider id / URI scheme，并交给 `qubit-spi` 管理：

- `file` provider 可声明 id 为 `local`，alias 为 `file`
- OSS provider 可声明 id 为 `oss`
- S3 provider 可声明 id 为 `s3`
- 私有存储 provider 可声明 id 为 `company-store`

`qubit-fs` 不关心 provider 集合，只关心 provider 是否实现了统一 trait。

## 4. `std::path::Path` 能不能作为统一 Path

结论：不能作为跨后端统一路径类型，但可以作为本地后端内部实现类型。

`std::path::Path` 的语义是操作系统本地路径：

- Windows 有盘符、UNC、`\` 分隔符和平台路径前缀。
- Unix path 允许非 UTF-8 字节序列。
- `Path` 组件规范化跟本地平台强绑定。
- OSS/HDFS/FTP 的路径更接近 URI path 或 object key，不具备完整本地路径语义。
- OSS 中的 `a/../b` 可能只是普通 key 文本，不一定表示目录回退。

因此核心 API 应定义自己的路径模型，并区分两个层次：

```text
FsUri  = 完整定位信息：scheme + authority + path + query
FsPath = 某个 FileSystem 实例内的路径：统一 UTF-8、统一 "/" 分隔、无 provider 枚举
```

示例映射：

| 输入 URI | provider | authority | FsPath |
| --- | --- | --- | --- |
| `file:///var/log/app.log` | `file` | 无 | `/var/log/app.log` |
| `oss://my-bucket/reports/a.csv` | `oss` | `my-bucket` | `/reports/a.csv` |
| `hdfs://nn1:8020/user/a.parquet` | `hdfs` | `nn1:8020` | `/user/a.parquet` |
| `ftp://host/pub/readme.txt` | `ftp` | `host` | `/pub/readme.txt` |
| `webdav://host/dav/readme.txt` | `webdav` | `host` | `/dav/readme.txt` |

本地实现内部再做转换：

```rust
impl LocalFileSystem {
    fn to_local_path(&self, path: &FsPath) -> Result<PathBuf, FsError> {
        // 将 provider-local FsPath 映射到本地 root 下的 PathBuf。
        // 这里处理平台分隔符、root sandbox、防目录穿越等问题。
    }
}
```

跨后端 API 不承诺完整保留本地非 UTF-8 路径。如果确实需要支持 Unix 非 UTF-8 文件名，建议在 `qubit-fs-local` 中提供本地扩展 trait，而不是污染核心抽象。

## 5. 路径模型

### 5.1 `FsUri`

`FsUri` 表示可解析的完整资源位置。

建议字段：

```rust
pub struct FsUri {
    scheme: String,
    authority: Option<FsAuthority>,
    path: FsPath,
    query: Metadata,
}
```

设计约束：

- `scheme` 使用小写规范化字符串，不使用枚举。
- `authority` 保存 host、bucket、namenode、user-info 等定位信息，但凭据不直接放入明文字符串。
- `path` 使用 `FsPath`。
- `query` 可用 `qubit_metadata::Metadata` 保存非敏感连接选项，例如 region、namespace、profile 名称。
- 密钥、token、password 应通过凭据引用传入，不应长期保存在 `FsUri` 中。

### 5.2 `FsPath`

`FsPath` 是 provider-local path，不知道 provider 类型。

建议语义：

- UTF-8 字符串模型。
- `/` 是唯一分隔符。
- 可区分 absolute 与 relative。
- 空 path、`.`、`..` 的处理由 normalize policy 明确控制。
- 不保存 scheme、authority、bucket、host。
- 不直接等同于 URL path，因为部分后端需要保留 `%2F` 这样的 key 字符。

建议 API：

```rust
pub struct FsPath {
    absolute: bool,
    normalized: String,
}

impl FsPath {
    pub fn parse(path: &str) -> Result<Self, FsError>;
    pub fn root() -> Self;
    pub fn is_absolute(&self) -> bool;
    pub fn as_str(&self) -> &str;
    pub fn join(&self, child: &str) -> Result<Self, FsError>;
    pub fn parent(&self) -> Option<FsPath>;
    pub fn file_name(&self) -> Option<&str>;
    pub fn file_extension(&self) -> Option<&str>;
}
```

对 `..` 的建议：

- `FsPath::parse_normalized` 默认拒绝会逃逸 root 的 `..`。
- `FsPath::parse_literal` 用于 object store key 场景，允许把 `..` 当作普通组件。
- `FileSystem` 实例可以声明自己的 `PathSemantics`，例如 `Hierarchical` 或 `ObjectKey`。

## 6. 核心 trait 设计

MVP 推荐先做同步、对象安全的核心 trait，异步 API 作为后续平行 trait 或 feature。原因是现有 `qubit-io` 提供的是 `std::io` 能力组合，`qubit-local-files` 也是同步工具；如果一开始强制 tokio，会把核心抽象和运行时绑定过早。

### 6.1 `FileSystem`

```rust
pub trait FileSystem: std::fmt::Debug + Send + Sync {
    fn capabilities(&self) -> FileSystemCapabilities;

    fn metadata(&self) -> FileSystemMetadata;
    fn path_metadata(&self, path: &FsPath) -> FsResult<FileMetadata>;
    fn exists(&self, path: &FsPath) -> FsResult<bool>;
    fn list(&self, path: &FsPath, options: &ListOptions) -> FsResult<Box<dyn DirectoryStream>>;

    fn open_reader(&self, path: &FsPath, options: &ReadOptions)
        -> FsResult<Box<dyn FileReader>>;

    fn open_writer(&self, path: &FsPath, options: &WriteOptions)
        -> FsResult<Box<dyn FileWriter>>;

    fn create_dir(&self, path: &FsPath, options: &CreateDirOptions) -> FsResult<()>;
    fn delete(&self, path: &FsPath, options: &DeleteOptions) -> FsResult<()>;
    fn rename(&self, from: &FsPath, to: &FsPath, options: &RenameOptions) -> FsResult<()>;
    fn copy(&self, from: &FsPath, to: &FsPath, options: &CopyOptions) -> FsResult<CopyOutcome>;
}
```

原则：

- `FileSystem` 不使用泛型方法，保持 trait object 友好。
- 复杂操作统一使用 option 类型，避免参数列表膨胀。
- 对后端不支持的操作，返回 `FsErrorKind::UnsupportedOperation`，并在 `capabilities()` 中提前声明。
- `exists` 不应吞掉权限、网络、认证错误；只有确定不存在才返回 `Ok(false)`。

### 6.2 Reader / Writer

读接口可以直接复用标准库和 `qubit-io`：

```rust
pub trait FileReader: std::io::Read + Send {
    fn metadata(&self) -> Option<&FileMetadata>;
}
```

写接口不建议只暴露 `Write`，因为远端后端通常有 multipart upload、commit、abort 等生命周期。

```rust
pub trait FileWriter: std::io::Write + Send {
    fn commit(self: Box<Self>) -> FsResult<WriteOutcome>;
    fn abort(self: Box<Self>) -> FsResult<()>;
}
```

便利方法可以由 `FileSystemExt` 提供：

```rust
pub trait FileSystemExt {
    fn read_all(&self, path: &FsPath) -> FsResult<Vec<u8>>;
    fn write_all(&self, path: &FsPath, bytes: &[u8]) -> FsResult<WriteOutcome>;
}
```

`write_all` 内部负责 `open_writer`、`write_all`、`commit`；如果写入失败则尽量 `abort`。

## 7. 能力模型

不同后端能力差异很大。不要假设所有 provider 都支持完整 POSIX 语义。

建议能力字段：

```rust
pub struct FileSystemCapabilities {
    pub hierarchical_paths: bool,
    pub directories: bool,
    pub empty_directories: bool,
    pub symlinks: bool,
    pub range_read: bool,
    pub append: bool,
    pub random_write: bool,
    pub atomic_rename: bool,
    pub atomic_replace: bool,
    pub conditional_write: bool,
    pub server_side_copy: bool,
    pub recursive_delete: bool,
    pub temp_file: bool,
    pub temp_dir: bool,
    pub temp_persist: bool,
    pub temp_persist_atomic: bool,
    pub native_metadata: bool,
}
```

能力不是权限检查，只描述后端模型。权限、ACL、quota、对象锁等运行时结果仍然通过 `FsError` 表达。

典型示例：

| 能力 | Local | FTP | OSS/S3 | HDFS |
| --- | --- | --- | --- | --- |
| 层级目录 | 是 | 是 | 弱语义前缀 | 是 |
| 空目录 | 是 | 是 | 通常不原生支持 | 是 |
| range read | 是 | 不稳定 | 是 | 是 |
| append | 是 | 取决于服务端 | 通常否 | 取决于配置 |
| atomic rename | 同卷通常是 | 取决于服务端 | 否 | 是 |
| atomic replace | `qubit-local-files` 可支持 | 取决于服务端 | 条件 put 替代 | 取决于配置 |
| server-side copy | 否 | 否 | 是 | 取决于实现 |

## 8. 元信息模型

`FileMetadata` 应分为稳定字段与扩展字段。

```rust
pub struct FileMetadata {
    pub kind: FileKind,
    pub len: Option<u64>,
    pub modified_at: Option<SystemTime>,
    pub created_at: Option<SystemTime>,
    pub accessed_at: Option<SystemTime>,
    pub etag: Option<String>,
    pub content_type: Option<String>,
    pub checksum: Option<Checksum>,
    pub user_metadata: Metadata,
    pub provider_metadata: Metadata,
}
```

设计原则：

- 稳定字段只放跨后端常见语义。
- 不稳定或后端专属字段放进 `qubit_metadata::Metadata`。
- `user_metadata` 表示用户可设置的对象元数据。
- `provider_metadata` 表示后端返回的诊断信息，例如 OSS storage class、HDFS replication、FTP permissions。
- 如果某些 metadata 需要 schema 校验，可以复用 `MetadataSchema`。

`FileKind` 建议：

```rust
pub enum FileKind {
    File,
    Directory,
    Symlink,
    Object,
    Prefix,
    Other(String),
}
```

`Object` 和 `Prefix` 用于对象存储弱目录语义；`Other(String)` 给第三方后端留扩展空间。

## 9. 错误模型

`FsError` 必须统一封装后端差异，同时保留足够诊断上下文。

```rust
pub struct FsError {
    pub kind: FsErrorKind,
    pub operation: FsOperation,
    pub path: Option<FsPath>,
    pub target: Option<FsPath>,
    pub provider: Option<String>,
    pub message: String,
    pub source: Option<Box<dyn std::error::Error + Send + Sync>>,
}
```

`FsErrorKind` 建议：

```rust
pub enum FsErrorKind {
    NotFound,
    AlreadyExists,
    NotDirectory,
    IsDirectory,
    InvalidPath,
    PermissionDenied,
    AuthenticationFailed,
    ProviderUnavailable,
    UnsupportedOperation,
    Conflict,
    PreconditionFailed,
    Timeout,
    Interrupted,
    QuotaExceeded,
    DataCorruption,
    Io,
    Other,
}
```

错误设计原则：

- 不直接把 `std::io::Error` 作为公开统一错误。
- 本地实现可以从 `io::ErrorKind` 映射到 `FsErrorKind`。
- 远端实现应把 HTTP status、FTP reply、HDFS status 映射到统一 kind，并把原始错误放到 source。
- `UnsupportedOperation` 要区分“后端模型不支持”和“当前权限不允许”。
- `NotFound` 只表示后端明确确认不存在，不应用来吞掉认证失败或网络失败。

## 10. Provider SPI 设计

`rs-fs` 应复用 `qubit-spi`，不要自建另一套 provider registry。

### 10.1 ServiceSpec

```rust
pub struct FileSystemSpec;

impl ServiceSpec for FileSystemSpec {
    type Config = FileSystemConfig;
    type Output = Arc<dyn FileSystem>;
}
```

`FileSystemConfig` 保存 provider 创建文件系统实例所需的上下文：

```rust
pub struct FileSystemConfig {
    pub uri: FsUri,
    pub options: Metadata,
    pub credentials: Option<CredentialRef>,
}
```

`CredentialRef` 不直接等同于 secret 值。它可以表示环境变量名、profile 名称、外部凭据提供者 id，避免路径字符串和 debug 输出泄漏密钥。

### 10.2 Provider

后端 provider 实现 `ServiceProvider<FileSystemSpec>`：

```rust
use std::sync::Arc;

use qubit_spi::error::ProviderCreationError;

#[derive(Debug)]
pub struct LocalFileSystemProvider;

impl ServiceProvider<FileSystemSpec> for LocalFileSystemProvider {
    fn create(
        &self,
        config: &FileSystemConfig,
    ) -> Result<Arc<dyn FileSystem>, ProviderCreationError> {
        // 解析 file:// URI，创建 LocalFileSystem
    }
}

impl ProviderDefinition<FileSystemSpec> for LocalFileSystemProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("local").expect("valid provider ID"),
        )
        .with_aliases(["file"])
        .expect("valid provider aliases")
    }
}
```

注册与使用：

```rust
let registry = FileSystemRegistry::default();
qubit_fs_local::register_provider(&registry)?;
qubit_fs_oss::register_provider(&registry)?;

let uri = FsUri::parse("oss://bucket/reports/2026/a.csv")?;
let fs = registry.fs(&uri)?;
let resource = registry.resource(&uri)?;
```

### 10.3 显式发现 vs 自动发现

当前 `qubit-spi` 是显式注册模型，不做 Java `ServiceLoader` 式自动发现。这一点适合 `rs-fs` MVP：

- 应用依赖哪些后端，就显式注册哪些 provider。
- 测试可以只注册 mock provider，隔离更容易。
- 没有 linker magic，构建和排错更直接。

如果后续确实需要“依赖了后端 crate 就自动注册”，建议在 `qubit-fs` 或单独 `qubit-fs-inventory` feature 中叠加 `inventory` / `linkme`，不要把自动发现作为核心唯一机制。

## 11. Registry builder、Registry 与 FileResource

`FileSystemRegistry` 是 `ProviderRegistry<FileSystemSpec>` 的领域门面：

```rust
pub struct FileSystemRegistry {
    providers: ProviderRegistry<FileSystemSpec>,
}
```

职责：

- 持有 clone 共享状态的运行时 provider registry。
- 接受实现 `ProviderDefinition<FileSystemSpec>` 的自描述 provider。
- 按 `ProviderSelection` 解析出 `ResolvingServiceProvider<FileSystemSpec>`。
- 通过 scheme 解析 provider。
- 根据 `FsUri` 构造 `FileSystemConfig`。
- 调用 `ServiceProvider::create` 获得 `Arc<dyn FileSystem>`。
- 通过 `resource(&FsUri)` 返回绑定文件系统和 provider-local path 的 `FileResource`。
- 分别将 provider 选择错误与 provider 创建错误映射成 `FsError`，并保留原始 source。

`FileSystemRegistryBuilder` 只是可选的流式组装接口；构建出的 registry 仍可在运行时注册：

```rust
let registry = FileSystemRegistry::default();
qubit_fs_local::register_provider(&registry)?;

let uri = FsUri::parse("file:///tmp/report.csv")?;
let fs = registry.fs(&uri)?;
let resource = registry.resource(&uri)?;
```

应用可以把 `FileSystemRegistry` 放入进程级全局入口，并在启动时注册 provider。
下游库持有的 registry clone 会观察到相同注册结果，但通用 `qubit-fs` crate
不强制采用某一种全局单体实现。

`FileResource` 负责把完整 URI 拆成文件系统实例与 provider-local path 后形成资源对象：

```rust
pub struct FileResource {
    fs: Arc<dyn FileSystem>,
    path: FsPath,
}

impl FileResource {
    pub fn fs(&self) -> &dyn FileSystem;
    pub fn path(&self) -> &FsPath;
    pub fn exists(&self) -> FsResult<bool>;
    pub fn metadata(&self) -> FsResult<FileMetadata>;
    pub fn read_all(&self) -> FsResult<Vec<u8>>;
    pub fn write_all(&self, bytes: &[u8]) -> FsResult<WriteOutcome>;
}
```

为什么要拆分：

- 同一个 `oss://bucket` 可以复用一个 `OssFileSystem` client。
- 操作接口只接收 `FsPath`，不会每次重复解析 scheme、host、bucket。
- registry 可以缓存 `Arc<dyn FileSystem>`，缓存 key 为 provider id + authority + options 摘要。

如果启用 `registry-cache` feature，可以用 `qubit-atomic::ArcAtomicRef` 热替换 registry/cache 快照；核心 API 不应强制依赖这套并发策略。

## 12. 本地文件系统实现边界

`qubit-fs-local` 应复用 `qubit-local-files`：

- `Files::ensure_parent` 用于写文件前创建父目录。
- `Files::atomic_write` / `atomic_write_with` 用于 durable same-directory atomic replace。
- `Files::copy_dir_all_with` 用于递归复制。
- `TempFile` / `TempDir` 可用于测试和 staged write。
- `Filenames` 可用于本地路径 lexical helper，但不应直接暴露为核心 `FsPath` 语义。

本地实现建议支持：

- root sandbox：`LocalFileSystem::new(root: PathBuf)`，所有 `FsPath` 映射到 root 下。
- explicit absolute mode：只有 `file:///abs/path` 或明确配置时允许绝对路径。
- 防目录穿越：规范化后不得逃出 root。
- symlink policy：默认不跟随删除目录中的 symlink；与 `qubit-local-files` 现有策略对齐。
- atomic replace：优先用 `Files::atomic_write`。

## 13. 对象存储实现边界

OSS/S3 这类对象存储不应伪装成完整 POSIX 文件系统。

建议语义：

- `FsPath` 映射为 object key。
- `Directory` 更多是 prefix 视图，不代表真实目录 inode。
- `create_dir` 可以创建 marker object，也可以根据配置声明不支持空目录。
- `rename` 默认不声明 atomic；实现上通常是 copy + delete。
- `copy` 优先使用 server-side copy。
- `metadata` 中 `etag`、`content_type`、storage class 放在稳定字段或 `provider_metadata`。
- `open_writer` 必须显式 `commit`；multipart upload 失败时应 `abort`。

这类差异通过 `FileSystemCapabilities` 和错误模型暴露，而不是在文档里假装所有后端都一样。

## 14. HDFS / FTP 实现边界

HDFS：

- 路径通常是强层级语义。
- `rename` 可以是重要的 atomic commit 原语。
- append 是否可用取决于集群配置，应在 `capabilities()` 中反映。
- 权限、owner、group、replication、block size 放进 `provider_metadata`。

FTP：

- 协议能力取决于服务器和扩展命令。
- `metadata` 可能只能得到不完整信息。
- `rename` 可能存在，但 atomic 语义不稳定。
- 连接池和 keepalive 是 provider 实现细节，不进入核心 trait。

## 15. WebDAV 实现边界

`rs-fs-webdev` 是 WebDAV 协议后端 crate。crate 名可以沿用 `webdev`，但文档和公开描述中应明确协议名是 WebDAV，默认支持 `webdav://` 和 `webdavs://` 两类 scheme。

建议 provider 形状：

```rust
#[derive(Debug)]
pub struct WebDavFileSystemProvider;

impl ProviderDefinition<FileSystemSpec> for WebDavFileSystemProvider {
    fn descriptor(&self) -> ProviderDescriptor {
        ProviderDescriptor::new(
            ProviderId::new("webdav")
                .expect("WebDAV provider ID should be valid"),
        )
        .with_aliases(["webdavs", "webdev"])
        .expect("WebDAV provider aliases should be valid")
    }
}

pub fn register_provider(registry: &FileSystemRegistry) -> FsResult<()> {
    registry.register(WebDavFileSystemProvider)
}
```

URI 映射：

```text
webdav://example.com/dav/docs/a.txt   -> http://example.com/dav/docs/a.txt
webdavs://example.com/dav/docs/a.txt  -> https://example.com/dav/docs/a.txt
```

`webdev` 作为 alias 可以兼容 crate 命名或用户输入，但 canonical provider id 建议使用标准协议名 `webdav`。

WebDAV 后端建议能力：

- `metadata` 通过 `PROPFIND Depth: 0` 获取资源属性。
- `list` 通过 `PROPFIND Depth: 1` 获取直接子项；递归 list 可以循环分页/递归请求，但不能假设所有服务端表现一致。
- `open_reader` 使用 `GET`，range read 可映射到 HTTP `Range`。
- `open_writer` 使用 `PUT`；如果需要显式 `commit`，writer 可以先写入临时资源，再在 `commit` 时执行 `MOVE`。
- `create_dir` 使用 `MKCOL`。
- `delete` 使用 `DELETE`。
- `rename` / `move` 使用 `MOVE`；`copy` 使用 `COPY`。
- 条件写入可以映射 HTTP `If-Match` / `If-None-Match`，失败返回 `PreconditionFailed`。

WebDAV 与本地文件系统的关键差异：

- WebDAV 是 HTTP 语义，不应承诺完整 POSIX 行为。
- `MOVE` 在单服务端同一命名空间内通常可用，但是否 atomic 取决于服务端实现；`capabilities.atomic_rename` 默认应保守为 `false`，除非 provider 针对具体服务端确认。
- 空目录通常可通过 `MKCOL` 表达，比对象存储 prefix 更接近层级目录。
- 锁能力通过 WebDAV `LOCK` / `UNLOCK` 表达，但 MVP 不建议把锁放入核心 `FileSystem` trait；可以作为 `qubit-fs-webdev` 的后端扩展 trait。
- ETag 是 WebDAV/HTTP 中重要的并发控制信息，应映射到 `FileMetadata.etag`。
- 服务端自定义属性可以放进 `provider_metadata`，不要扩展核心 `FileMetadata` 字段。

WebDAV 错误映射建议：

| HTTP 状态 | FsErrorKind |
| --- | --- |
| 401 | `AuthenticationFailed` |
| 403 | `PermissionDenied` |
| 404 | `NotFound` |
| 405 | `UnsupportedOperation` |
| 409 | `Conflict` |
| 412 | `PreconditionFailed` |
| 423 | `Conflict` 或后续扩展 `Locked` |
| 507 | `QuotaExceeded` |

凭据建议：

- 支持 `CredentialRef::DefaultChain`、`Profile`、`Environment` 和 `Provider`。
- Basic、Digest、Bearer token、mTLS 等认证方式作为 provider option 或 credential provider 细节处理。
- URI 中的 username 可以作为定位 hint，但 password/token 默认拒绝进入持久配置和日志。

`qubit-fs-webdev` 初始版本建议先实现：

- `metadata`
- `list` 非递归
- `open_reader`
- `write_all` / `open_writer` + `commit`
- `create_dir`
- `delete`
- `rename` 尽力执行
- `copy` 尽力执行

暂不进入 MVP 的 WebDAV 能力：

- WebDAV lock 生命周期管理。
- DeltaV / versioning。
- ACL 完整模型。
- 服务端特定属性 schema。

## 16. 临时资源抽象与资源边界决策

`TempFile` 和 `TempDir` 应作为 `rs-fs` 抽象层的一等接口存在，但它们的核心语义不是“某个路径的薄包装”，而是“拥有清理责任的临时资源句柄”。

如果没有自动清理语义，调用 `TempFile::cleanup()` 与调用 `FileSystem::delete(path)` 区别不大，`TempFile` / `TempDir` 的抽象价值会明显下降。因此核心层应明确以下语义：

- 临时资源创建后由句柄拥有清理责任。
- 显式 `cleanup()` 释放资源。
- `Drop` 兜底执行尽力清理。
- `persist()` 把临时资源转为正式资源，并解除临时清理责任。
- `keep()` 解除自动清理责任，把临时路径交还给调用方。
- 临时资源可以携带后端 native 状态，例如本地文件句柄、multipart upload id、WebDAV lock token、HDFS lease 或内存 entry id。

### 16.1 为什么引入 `TempResource`

`TempFile` 和 `TempDir` 有明显的公共生命周期语义：

```rust
pub trait TempResource: std::fmt::Debug + Send + Sync {
    fn fs(&self) -> Arc<dyn FileSystem>;
    fn path(&self) -> &FsPath;

    fn resource(&self) -> FileResource {
        FileResource::new(self.fs(), self.path().clone())
    }

    fn exists(&self) -> FsResult<bool> {
        self.fs().exists(self.path())
    }

    fn metadata(&self) -> FsResult<FileMetadata> {
        self.fs().path_metadata(self.path())
    }

    fn cleanup(self: Box<Self>) -> FsResult<()>;
    fn keep(self: Box<Self>) -> FsResult<FsPath>;
}
```

`TempResource` 的职责是承载“临时资源共有行为”，包括：

- 返回所属 `FileSystem`。
- 返回 provider-local `FsPath`。
- 转为普通 `FileResource`。
- 查询存在性和元数据。
- 执行显式清理。
- 放弃清理责任并保留路径。

`fs()` 返回 `Arc<dyn FileSystem>`，而不是 `&dyn FileSystem`，是为了让默认方法可以构造 `FileResource`，也便于调用方把临时资源转换为普通资源后继续使用。

### 16.2 为什么不把 `TempFile` 和 `TempDir` 完全合并

不建议只保留一个统一的 `TempResource` 类型来同时表示文件和目录。原因是两者的 `persist()` 结果不同：

```rust
pub trait TempFile: TempResource {
    fn persist(
        self: Box<Self>,
        target: &FsPath,
        options: &PersistOptions,
    ) -> FsResult<WriteOutcome>;
}

pub trait TempDir: TempResource {
    fn persist(
        self: Box<Self>,
        target: &FsPath,
        options: &PersistOptions,
    ) -> FsResult<()>;
}
```

如果强行合并，通常需要引入：

```rust
pub enum TempResourceKind {
    File,
    Directory,
}

pub enum TempPersistOutcome {
    File(WriteOutcome),
    Directory,
}
```

这会让调用方在已经知道自己创建的是临时文件时，仍然被迫处理目录结果分支，类型语义反而变弱。

推荐结构是：

```rust
pub trait TempResource: std::fmt::Debug + Send + Sync {
    // 公共生命周期能力
}

pub trait TempFile: TempResource {
    // 文件专属能力
}

pub trait TempDir: TempResource {
    // 目录专属能力
}
```

这样既消除 `path()`、`cleanup()`、`keep()` 等重复定义，又保留文件和目录在编译期的语义边界。

### 16.3 `TempFile` 的便利读写方法

`TempFile` 不应要求实现 `std::io::Write`。原因是本地临时文件可以天然持有打开的 `std::fs::File`，但 OSS/WebDAV/HDFS 中的临时文件可能只是临时 key、临时路径、multipart 会话或远端锁。把 `TempFile` 直接建模成 `Write` 会把不同后端的生命周期语义压扁。

但 `TempFile` 应提供常用读写便利方法。这些方法默认委托给所属 `FileSystem`，具体后端可以在有收益时覆盖：

```rust
pub trait TempFile: TempResource {
    fn open_reader(&self, options: &ReadOptions) -> FsResult<Box<dyn FileReader>> {
        self.fs().open_reader(self.path(), options)
    }

    fn open_writer(&self, options: &WriteOptions) -> FsResult<Box<dyn FileWriter>> {
        self.fs().open_writer(self.path(), options)
    }

    fn read_all(&self) -> FsResult<Vec<u8>> {
        self.fs().read_all(self.path())
    }

    fn write_all(&self, bytes: &[u8]) -> FsResult<WriteOutcome> {
        self.fs().write_all(self.path(), bytes)
    }

    fn persist(
        self: Box<Self>,
        target: &FsPath,
        options: &PersistOptions,
    ) -> FsResult<WriteOutcome>;
}
```

这样调用方可以直接写：

```rust
let temp = TempResources::create_default_file(fs.clone())?;
temp.write_all(b"hello")?;
let output = temp.read_all()?;
temp.persist(&FsPath::parse("/final.txt")?, &PersistOptions::default())?;
```

而不是每次都写：

```rust
fs.write_all(temp.path(), b"hello")?;
```

设计边界是：`TempFile` 提供临时文件最常用的文件操作便利方法，但不替代完整的 `FileSystem`。复杂复制、重命名、删除等普通资源操作仍然可以通过 `temp.resource()` 转成 `FileResource` 后执行。

### 16.4 `TempDir` 的便利目录方法

`TempDir` 应提供目录相关便利方法，默认同样委托给所属 `FileSystem`：

```rust
pub trait TempDir: TempResource {
    fn list(&self, options: &ListOptions) -> FsResult<Box<dyn DirectoryStream>> {
        self.fs().list(self.path(), options)
    }

    fn child(&self, name: &str) -> FsResult<FileResource> {
        Ok(FileResource::new(self.fs(), self.path().join(name)?))
    }

    fn create_child_dir(
        &self,
        name: &str,
        options: &CreateDirOptions,
    ) -> FsResult<FileResource> {
        let child = self.child(name)?;
        child.create_dir(options)?;
        Ok(child)
    }

    fn persist(
        self: Box<Self>,
        target: &FsPath,
        options: &PersistOptions,
    ) -> FsResult<()>;
}
```

`TempDir` 的核心价值是临时目录树的生命周期管理。对象存储后端未必支持真正的空目录，因此 `TempDir` 能力必须通过 `FileSystemCapabilities.temp_dir` 声明。

### 16.5 为什么 `FileResource` 暂不拆成 `FsFile` / `FsDir`

`FileResource` 表示“已经通过 registry 解析出来的文件系统资源”，不是“确定存在的普通文件”。它可能对应：

- 普通文件。
- 目录。
- symlink。
- 对象存储 object。
- 对象存储 prefix。
- WebDAV collection。
- 还不存在但准备写入的路径。

在很多后端上，URI 本身不能静态决定资源类型。比如 `oss://bucket/a/b` 可能是 object、prefix，也可能不存在。如果把 `FileResource` 拆成 `FsFile` / `FsDir`，解析阶段要么提前做 metadata I/O，要么暴露额外的运行时转换 API，反而增加复杂度。

因此 MVP 保持：

```rust
pub struct FileResource {
    fs: Arc<dyn FileSystem>,
    path: FsPath,
}
```

如果后续确实需要强类型资源，可以在 `FileResource` 上增加校验型转换，而不是替代它：

```rust
impl FileResource {
    pub fn require_file(&self) -> FsResult<FsFile>;
    pub fn require_dir(&self) -> FsResult<FsDir>;
}
```

这些方法应通过 `metadata()` 验证类型，因此属于增强 API，不应成为基础解析路径。

### 16.6 为什么 `FileResource` 暂不 trait 化

不建议把 `FileResource` 设计成 trait 并要求每个后端提供 native resource 实现。原因是 `FileResource` 当前只是定位器和委托器：

```text
FileResource = Arc<dyn FileSystem> + FsPath
```

它通常不拥有独立生命周期，也不携带创建过程状态。把它 trait 化会带来额外成本：

- `FileSystemRegistry::resource()` 返回 trait object 后，clone、debug、路径访问和普通使用都会更重。
- 每个 `FileSystem` 都要额外考虑 resource factory，但大多数实现只是转调 `FileSystem`。
- 如果 native resource 缓存 metadata，会马上遇到缓存失效、并发可见性和条件写入语义问题。
- 普通资源的优化空间更适合沉到 `FileSystem` 方法内部，而不是扩展成第二套 resource trait。

因此推荐保持：

```rust
FileResource = concrete wrapper
TempResource = common lifecycle trait
TempFile = temp file trait
TempDir = temp dir trait
```

### 16.7 为什么 `TempFile` / `TempDir` 仍然值得保留为 trait

`TempFile` / `TempDir` 和 `FileResource` 的关键区别是：临时资源可能携带后端 native 状态。这些状态不是 `FsPath` 能表达的。

典型例子：

| 后端 | 临时资源可能携带的 native 状态 |
| --- | --- |
| Local | 文件句柄、真实临时路径、是否保留、是否执行 drop 清理 |
| OSS/S3/MinIO | multipart upload id、bucket/key、已上传 parts、etag、临时 object key |
| HDFS | write lease、block size、replication、临时目录提交协议状态 |
| FTP/SFTP | 连接池 session、远端错误码上下文、server-side rename 能力 |
| WebDAV | lock token、etag、collection marker、临时资源 URI |
| Memory FS | entry id、buffer handle、内部 map slot |

这些状态会直接影响清理、提交和性能。因此 `TempResourceFactory` 返回 `Box<dyn TempFile>` / `Box<dyn TempDir>` 是有实际收益的。

### 16.8 具体后端可以覆盖哪些方法

默认实现已经能覆盖大多数后端：

- 通过 `FileSystem::open_writer()` 写入临时路径。
- 通过 `FileSystem::delete()` 清理临时资源。
- 通过 `FileSystem::rename()` 优先提交。
- 必要时按 `PersistOptions` 降级到 `copy + delete`。

后端只有在能提供更低成本、更安全或更原子的语义时才需要覆盖。

Local 后端可以优化：

- `TempFile::open_writer()` 复用创建临时文件时已经打开的 `std::fs::File`，避免按路径重新打开。
- `TempFile::persist()` 使用同目录 `rename` 实现真正原子发布。
- `TempFile::cleanup()` 使用本地临时文件句柄和真实路径做清理。
- `TempDir::persist()` 使用本地目录 rename，避免递归 copy。

OSS/S3/MinIO 后端可以优化：

- `TempFile::open_writer()` 直接开启 multipart upload session。
- `TempFile::write_all()` 小文件直接 `PutObject` 到临时 object，大文件自动 multipart。
- `TempFile::persist()` 使用服务端 `CopyObject` 从临时 object 复制到目标，然后删除临时 object。
- `TempFile::cleanup()` 如果 multipart 未完成，调用 `AbortMultipartUpload`，而不是只删除 object。
- `TempDir::cleanup()` 批量删除 prefix 下对象，避免逐个普通 delete。

HDFS 后端可以优化：

- `TempFile::open_writer()` 用 HDFS create API 创建临时文件并持有写 lease。
- `TempFile::persist()` 在同一 namespace 内使用 HDFS rename，通常是 metadata 操作。
- `TempDir::persist()` rename 整个目录树，避免递归 copy。
- `cleanup()` 删除临时路径并释放可能存在的 lease。

FTP/SFTP 后端可以优化：

- `TempFile::open_writer()` 复用连接池 session 执行 `STOR temp_path`。
- `persist()` 使用 FTP `RNFR/RNTO` 或 SFTP rename。
- `cleanup()` 使用 `DELE` / `RMD`，并把服务端错误码映射为更准确的 `FsErrorKind`。
- `TempDir::list()` 复用同一连接做目录列举，避免重复建连和认证。

WebDAV 后端可以优化：

- `TempFile::persist()` 使用 WebDAV `MOVE`，避免下载再上传。
- `TempFile::cleanup()` 带 lock token 解锁并删除临时资源。
- `TempDir::list()` 使用 `PROPFIND` 一次性获取目录项和属性。
- `metadata()` 复用创建或写入时获得的 ETag / Last-Modified。

MemoryFileSystem 后端可以优化：

- `TempFile::read_all()` 直接 clone 内部 buffer，不走 reader trait object。
- `TempFile::write_all()` 直接替换内存 entry。
- `persist()` 直接移动 map entry，不复制字节。
- `cleanup()` 通过内部 entry id 删除，避免 path 再查找。

真正最值得覆盖的方法是：

- `TempFile::open_writer()`
- `TempFile::write_all()`
- `TempFile::persist()`
- `TempFile::cleanup()`
- `TempDir::list()`
- `TempDir::persist()`
- `TempDir::cleanup()`

### 16.9 `ManagedTempFile` 与 `ManagedTempDir`

`ManagedTempFile` 是 `rs-fs` 核心层可直接提供的默认实现。它不依赖本地文件系统，而是在内部保存文件系统实例和临时路径：

```rust
pub struct ManagedTempFile {
    fs: Arc<dyn FileSystem>,
    path: FsPath,
    cleanup_on_drop: bool,
}
```

默认行为：

- 实现 `TempResource` 和 `TempFile`。
- 创建时由核心 helper 生成临时路径。
- 写入便利方法默认委托给 `FileSystemExt::write_all()`。
- `cleanup()` 调用 `fs.delete(path, DeleteOptions { missing_ok: true, .. })`。
- `persist()` 优先调用 `fs.rename(temp, target, RenameOptions { atomic: Required, .. })`。
- 如果 `PersistOptions` 允许降级，则可在 atomic rename 不支持时使用 `copy + delete`。
- `keep()` 将 `cleanup_on_drop` 置为 `false` 并返回 path。
- `Drop` 中如果 `cleanup_on_drop=true`，执行尽力删除。

`ManagedTempDir` 与 `ManagedTempFile` 类似：

```rust
pub struct ManagedTempDir {
    fs: Arc<dyn FileSystem>,
    path: FsPath,
    cleanup_on_drop: bool,
}
```

默认行为：

- 实现 `TempResource` 和 `TempDir`。
- 创建时调用 `fs.create_dir(temp_path, ...)`。
- `cleanup()` 调用递归 `delete`。
- `persist()` 优先使用 rename。
- 不支持 atomic rename 时，是否允许 copy + delete 由 `PersistOptions` 决定。
- `Drop` 中执行尽力递归删除。

`ManagedTempFile` / `ManagedTempDir` 的价值是让大多数后端无需从零实现临时资源生命周期。后端只要实现了 `FileSystem` 的基础操作，就能获得默认临时资源能力。

对于对象存储，`ManagedTempDir` 只有在 provider 明确支持目录或 prefix 递归操作时才应启用。否则 `FileSystemCapabilities.temp_dir=false`。

### 16.10 创建入口

`FileSystem` 应提供当前实例的临时资源 factory：

```rust
pub trait FileSystem {
    fn temp_resource_factory(&self) -> &dyn TempResourceFactory;
}
```

`TempResourceFactory` 只提供最强接口，直接接收完整 options 对象：

```rust
pub trait TempResourceFactory: std::fmt::Debug + Send + Sync {
    fn create_file(
        &self,
        owner: Arc<dyn FileSystem>,
        options: &TempFileOptions,
    ) -> FsResult<Box<dyn TempFile>>;

    fn create_dir(
        &self,
        owner: Arc<dyn FileSystem>,
        options: &TempDirOptions,
    ) -> FsResult<Box<dyn TempDir>>;

    fn make_temp_path(
        &self,
        parent: Option<&FsPath>,
        prefix: &str,
        suffix: &str,
    ) -> FsResult<FsPath>;
}
```

语义：

- `FileSystem` 实例自己决定返回 native factory 还是核心层默认 factory。
- 默认实现返回 `ManagedTempResourceFactory::shared()`。
- Local / OSS / WebDAV / HDFS 可以返回自己的 native factory。
- `TempResourceFactory` 不提供 prefix-only、default 等便捷方法；这些属于门面层。
- `TempResourceFactory::make_temp_path()` 提供默认临时路径命名格式，native factory 可以复用，也可以按 provider 语义自行生成。

同时提供独立 helper 作为推荐创建入口：

```rust
pub struct TempResources;

impl TempResources {
    pub fn create_file(
        fs: Arc<dyn FileSystem>,
        options: &TempFileOptions,
    ) -> FsResult<Box<dyn TempFile>>;

    pub fn create_dir(
        fs: Arc<dyn FileSystem>,
        options: &TempDirOptions,
    ) -> FsResult<Box<dyn TempDir>>;
}
```

`TempResources::create_file()` / `create_dir()` 的行为：

1. 先调用 `fs.temp_resource_factory()`。
2. 再把 `Arc<dyn FileSystem>` 和完整 options 传给 factory。
3. 具体 factory 决定返回 native `TempFile` / `TempDir`，还是创建 `ManagedTempFile` / `ManagedTempDir`。

`TempResources` 可以提供便捷方法，例如：

```rust
impl TempResources {
    pub fn create_default_file(fs: Arc<dyn FileSystem>) -> FsResult<Box<dyn TempFile>>;
    pub fn create_file_with_prefix(
        fs: Arc<dyn FileSystem>,
        prefix: &str,
    ) -> FsResult<Box<dyn TempFile>>;

    pub fn create_default_dir(fs: Arc<dyn FileSystem>) -> FsResult<Box<dyn TempDir>>;
    pub fn create_dir_with_prefix(
        fs: Arc<dyn FileSystem>,
        prefix: &str,
    ) -> FsResult<Box<dyn TempDir>>;
}
```

后端如果有更强实现，可以覆盖 `temp_resource_factory()`：

- Local 后端可以适配 `qubit-local-files` 的本地临时文件和临时目录。
- WebDAV 后端可以使用临时资源路径，`persist()` 时执行 `MOVE`。
- OSS 后端可以使用临时 object key 或 multipart upload session。
- HDFS 后端可以使用临时目录 + atomic rename 作为提交协议。

### 16.11 与 `qubit-local-files` 的命名关系

抽象层应占用通用名称：

```rust
qubit_fs::TempResource
qubit_fs::TempFile
qubit_fs::TempDir
qubit_fs::ManagedTempFile
qubit_fs::ManagedTempDir
```

本地工具 crate 中现有的 `TempFile` / `TempDir` 后续可以改名为：

```rust
qubit_local_files::LocalTempFile
qubit_local_files::LocalTempDir
```

如果保留三层结构：

```text
qubit-fs
qubit-local-files
qubit-fs-local
```

则 `qubit-local-files` 继续提供本地专用的 `LocalTempFile` / `LocalTempDir`，
`qubit-fs-local` 只负责适配并导出后端实现。等 `qubit-fs` API 稳定后，
再评估是否需要进一步的破坏性重命名。

## 17. 异步 API 规划

MVP 不建议把 `FileSystem` 直接定义为 async trait，原因：

- Rust 原生 async trait 的对象安全和生命周期设计仍需要包装。
- `async_trait` 会引入装箱 Future，后续很难无痛调整。
- `qubit-io` 和 `qubit-local-files` 当前是同步 std I/O 体系。
- 很多业务场景可以先用同步 trait + executor 隔离阻塞操作。

后续可以新增平行 trait：

```rust
pub trait AsyncFileSystem: std::fmt::Debug + Send + Sync {
    fn capabilities(&self) -> FileSystemCapabilities;
    // 返回 boxed future 或使用关联 Future 类型，具体等实现阶段再定。
}
```

异步 provider 使用独立 `AsyncFileSystemSpec`。不要把同步和异步硬塞进同一个 trait，否则每个后端都会被迫实现大量不自然的适配层。

## 18. 与现有工具 crate 的关系

### `qubit-spi`

用于自描述 provider 注册、别名、优先级、`ProviderSelection`、创建 fallback
和错误聚合。选择 provider 与用配置创建 service 是两个独立阶段。

### `qubit-io`

用于复用 I/O 能力组合与 stream helper：

- `ReadSeek`、`ReadWrite`、`ReadWriteSeek` 可用于可 seek 的本地 handle。
- `Streams::copy_at_most`、`Streams::content_eq` 可作为默认扩展方法实现的基础。
- `LimitReader`、`CountingReader` 等 wrapper 可用于 `read_all`、限流读取和统计。

### `qubit-local-files`

用于本地后端的 durable atomic write、临时文件、目录递归复制、清理和文件名 helper。

其中现有 `TempFile` / `TempDir` 的语义与抽象层临时资源非常接近。后续如果 `qubit_fs::TempFile` / `TempDir` 成为公共核心接口，建议将本地工具类型重命名为 `LocalTempFile` / `LocalTempDir`，或在 `qubit-fs-local` 中做适配层，避免类型名和语义层级混淆。

### `qubit-metadata`

用于可扩展元数据：

- provider-specific metadata
- user metadata
- list/filter options
- provider config options

如果元数据字段需要长期稳定，可以用 `MetadataSchema` 固化 schema。

### `qubit-atomic`

不建议成为核心 API 的必要依赖。它适合用于实现层：

- registry/cache 快照热替换
- 活跃 reader/writer 计数
- provider client 状态引用更新

如果实现确实需要，可通过 feature 打开。

## 19. MVP 范围

第一阶段建议只实现这些内容：

- `FsPath`、`FsUri` 解析与规范化。
- `FsError`、`FsErrorKind`、`FsResult`。
- `FileMetadata`、`FileKind`、`DirEntry`。
- `FileSystemCapabilities`。
- 同步 `FileSystem` trait。
- `FileReader`、`FileWriter`。
- `TempResource`、`TempFile`、`TempDir` 抽象接口。
- `ManagedTempFile`、`ManagedTempDir` 默认托管实现。
- `CopyOptions`、`CopyStats`、`CopyOutcome` 以及复制相关策略类型。
- `FileSystemSpec`、`FileSystemRegistry`。
- `qubit-fs-local` 作为第一个 provider。
- `qubit-fs-webdev` 作为第一个远程层级文件系统 provider，用于验证 HTTP/WebDAV 语义。
- mock / memory provider 用于测试 registry 与路径语义。

第一阶段暂不做：

- 自动 provider 发现。
- HDFS/OSS/FTP 真实 SDK 接入。
- async trait。
- FUSE / mount。
- ACL 完整模型。
- 跨后端事务。

## 20. 后续演进

第二阶段：

- `qubit-fs-oss`，覆盖对象存储能力差异。
- 完善 `qubit-fs-webdev` 的 lock、conditional write、服务端属性映射和 HTTPS 认证策略。
- range read、conditional write、server-side copy。
- provider cache 与连接生命周期管理。
- 文档补充 OSS/HDFS/FTP provider guide。

第三阶段：

- `AsyncFileSystem` 平行 API。
- inventory/linkme 自动注册 feature。
- mount table / virtual filesystem。
- 加密、压缩、缓存、审计 wrapper filesystem。
- 跨后端 copy pipeline，优先 server-side copy，降级为 stream copy。

## 21. 计划模块结构

`qubit-fs` 核心 crate 建议按职责拆模块，而不是在 `lib.rs` 堆所有类型：

```text
src/
  lib.rs
  path/
    fs_path.rs
    fs_uri.rs
    fs_authority.rs
    path_semantics.rs
  metadata/
    file_metadata.rs
    file_kind.rs
    dir_entry.rs
    checksum.rs
  error/
    fs_error.rs
    fs_error_kind.rs
    fs_operation.rs
  options/
    read_options.rs
    write_options.rs
    list_options.rs
    delete_options.rs
    rename_options.rs
    copy_options.rs
    copy_stats.rs
    copy_outcome.rs
  traits/
    file_system.rs
    file_reader.rs
    file_writer.rs
    directory_stream.rs
    file_system_ext.rs
  temp/
    temp_resource.rs
    temp_file.rs
    temp_dir.rs
    managed_temp_file.rs
    managed_temp_dir.rs
    temp_resources.rs
    temp_options.rs
  provider/
    file_system_config.rs
    file_system_provider.rs
    file_system_registry.rs
```

导出策略：

- crate root 只重导出主要公开 API。
- 具体文件显式导入依赖，避免通过 `mod.rs` + `use super::*` 隐式共享名称。
- `prelude` 暂不建议加入 MVP；等 API 稳定后再决定是否需要。
- 所有 provider 相关类型放在 `provider` 模块，`FileSystemProvider` 作为 `ProviderDefinition<FileSystemSpec>` 的 trait object alias 暴露。

## 22. 操作 option 设计

文件系统操作不要依赖越来越长的参数列表。每类操作使用独立 option 类型，并实现 `Default`。

### 22.1 `ReadOptions`

建议字段：

```rust
pub struct ReadOptions {
    pub offset: Option<u64>,
    pub length: Option<u64>,
    pub if_match: Option<String>,
    pub if_none_match: Option<String>,
    pub checksum: ChecksumPolicy,
}
```

语义：

- `offset/length` 表示 range read；不支持时返回 `UnsupportedOperation`。
- `if_match/if_none_match` 对应 ETag 或 provider 可映射的版本条件。
- `checksum` 表示是否要求读取后验证 checksum；MVP 可先只定义，不强制所有后端实现。

### 22.2 `WriteOptions`

建议字段：

```rust
pub struct WriteOptions {
    pub create_parent: bool,
    pub mode: WriteMode,
    pub content_type: Option<String>,
    pub user_metadata: Metadata,
    pub checksum: Option<Checksum>,
}

pub enum WriteMode {
    CreateNew,
    CreateOrTruncate,
    Append,
    ReplaceAtomic,
    ConditionalReplace { etag: String },
}
```

语义：

- `CreateNew`：目标存在则返回 `AlreadyExists`。
- `CreateOrTruncate`：目标存在则覆盖，不承诺 atomic。
- `Append`：仅在后端声明支持 append 时可用。
- `ReplaceAtomic`：后端必须承诺不可观察到半写状态；本地实现用 `qubit-local-files` 的 atomic write，OSS 可根据条件写或临时 key + commit 策略决定是否支持。
- `ConditionalReplace`：版本匹配才替换，失败返回 `PreconditionFailed`。

### 22.3 `ListOptions`

建议字段：

```rust
pub struct ListOptions {
    pub recursive: bool,
    pub follow_symlinks: bool,
    pub include_metadata: bool,
    pub page_size: Option<usize>,
    pub prefix: Option<String>,
}
```

语义：

- `recursive=false` 表示只列举直接子项或同级 prefix。
- `include_metadata=false` 时后端可只返回 path 和 file type，用于高性能列举。
- `page_size` 是 hint，不是强制要求；后端可以按服务端限制调整。
- `prefix` 是 provider-local 的名字前缀过滤，不是 glob。

### 22.4 `DeleteOptions`

建议字段：

```rust
pub struct DeleteOptions {
    pub recursive: bool,
    pub missing_ok: bool,
    pub if_match: Option<String>,
}
```

语义：

- 删除目录时 `recursive=false` 且目录非空，返回 `Conflict`。
- `missing_ok=true` 时明确不存在可返回 `Ok(())`；认证、权限、网络错误仍返回错误。
- `if_match` 用于对象版本或 ETag 条件删除。

### 22.5 `RenameOptions`

`RenameOptions`：

```rust
pub struct RenameOptions {
    pub overwrite: bool,
    pub atomic: AtomicityRequirement,
}

pub enum AtomicityRequirement {
    BestEffort,
    Required,
}
```

语义：

- `atomic=Required` 时，如果后端不能保证原子 rename，必须返回 `UnsupportedOperation`，不能偷偷降级。

## 23. 复制模型

`rs-fs` 应把复制操作建模成通用资源复制，而不是本地目录复制。`rs-local-files` 现有的 `CopyDirOptions` 和 `CopyDirStats` 可以作为起点，但上提到抽象层时需要扩展成完整跨后端模型。

核心类型建议：

```rust
pub struct CopyOptions {
    pub mode: CopyMode,
    pub conflict: CopyConflictPolicy,
    pub preserve_metadata: MetadataPreservePolicy,
    pub server_side: ServerSidePreference,
    pub follow_symlinks: bool,
    pub create_parent: bool,
    pub continue_on_error: bool,
    pub filter: Option<MetadataFilter>,
    pub progress: ProgressPolicy,
}
```

### 23.1 `CopyMode`

```rust
pub enum CopyMode {
    File,
    Tree,
    Auto,
}
```

语义：

- `File`：只复制单个文件、对象或资源；源是目录、prefix、collection 时返回 `IsDirectory` 或 `UnsupportedOperation`。
- `Tree`：复制目录树、prefix tree 或 collection subtree；源不是容器时可返回 `NotDirectory`，也可以由 provider 根据能力接受单文件树复制，但必须文档化。
- `Auto`：根据源 metadata 自动判断文件复制还是树复制，适合上层工具；核心实现需要多一次 metadata 查询。

### 23.2 `CopyConflictPolicy`

```rust
pub enum CopyConflictPolicy {
    Fail,
    Overwrite,
    Skip,
}
```

语义：

- `Fail`：目标存在时返回 `AlreadyExists` 或 `Conflict`。
- `Overwrite`：允许覆盖已有目标。
- `Skip`：目标存在时跳过该条目并增加 `CopyStats.skipped`。

这比单个 `overwrite: bool` 更完整。`rs-local-files` 现有 `overwrite=true` 可映射为 `Overwrite`，`overwrite=false` 可映射为 `Fail`。

### 23.3 `MetadataPreservePolicy`

`rs-local-files` 的 `preserve_permissions` 只适用于本地文件权限，不适合作为跨后端核心字段。抽象层应使用更通用的 metadata 保留策略：

```rust
pub enum MetadataPreservePolicy {
    None,
    Portable,
    UserMetadata,
    ProviderNative,
    All,
}
```

语义：

- `None`：只复制内容，不复制 metadata。
- `Portable`：复制 `content_type`、checksum、用户可移植 metadata 等跨后端稳定字段。
- `UserMetadata`：复制用户自定义 metadata。
- `ProviderNative`：尽量复制 provider-native metadata，例如本地权限、HDFS owner/group/permission、WebDAV 属性、OSS storage class。
- `All`：复制 provider 能表达的全部 metadata；如果目标后端不支持某些字段，按 provider 规则返回错误或降级。

如果 metadata preservation 是强要求，后续可在 `CopyOptions` 中增加 `metadata_required: bool`，或者把策略拆成 `BestEffort` / `Required` 两层。

### 23.4 `ServerSidePreference`

```rust
pub enum ServerSidePreference {
    Prefer,
    Require,
    Disable,
}
```

语义：

- `Prefer`：优先使用服务端 copy，例如 OSS copy object、WebDAV `COPY`、HDFS server-side copy；不可用时可以降级为 stream copy。
- `Require`：必须使用服务端 copy；不可用时返回 `UnsupportedOperation`。
- `Disable`：禁止服务端 copy，强制走 reader + writer pipeline。适合需要客户端加密、校验、审计或流式转换的场景。

### 23.5 symlink 与过滤

`follow_symlinks` 仅对支持 symlink 的后端有意义：

- `false`：遇到 symlink 时复制 symlink 本身；如果目标后端不支持 symlink，则返回 `UnsupportedOperation` 或按策略跳过。
- `true`：复制 symlink 指向的内容；provider 必须防止循环引用和逃逸 root。

`filter: Option<MetadataFilter>` 用于树复制时按 metadata 过滤条目。它复用 `qubit-metadata` 的过滤表达式，但 MVP 可以先不实现过滤，只在类型上预留。

### 23.6 `ProgressPolicy`

```rust
pub enum ProgressPolicy {
    None,
    CountOnly,
    Detailed,
}
```

语义：

- `None`：只返回最终 `CopyStats`。
- `CountOnly`：允许实现层统计文件数、目录数、字节数。
- `Detailed`：为后续 progress callback 或 event stream 预留；MVP 可以只定义不实现回调。

### 23.7 `CopyStats`

```rust
pub struct CopyStats {
    pub files: u64,
    pub directories: u64,
    pub symlinks: u64,
    pub objects: u64,
    pub prefixes: u64,
    pub bytes: u64,
    pub overwritten: u64,
    pub skipped: u64,
    pub failed: u64,
}
```

字段语义：

- `files`：复制的普通文件数量。
- `directories`：创建或复制的目录数量。
- `symlinks`：复制的 symlink 数量。
- `objects`：复制的对象存储 object 数量。
- `prefixes`：复制或创建的对象存储 prefix / WebDAV collection 数量。
- `bytes`：复制的内容字节数，不包含 metadata 开销。
- `overwritten`：覆盖已有目标的条目数量。
- `skipped`：因冲突策略、过滤条件或 provider 策略跳过的条目数量。
- `failed`：`continue_on_error=true` 时失败但继续处理的条目数量。

`rs-local-files` 现有 `CopyDirStats { files, directories, bytes }` 可以无损映射到新版 `CopyStats` 的子集。

### 23.8 `CopyOutcome`

```rust
pub struct CopyOutcome {
    pub stats: CopyStats,
    pub method: CopyMethod,
    pub diagnostics: Metadata,
}

pub enum CopyMethod {
    Local,
    ServerSide,
    Stream,
    Mixed,
}
```

语义：

- `Local`：本地后端内部复制，例如 `qubit-local-files` 递归复制。
- `ServerSide`：完全由服务端完成，例如 WebDAV `COPY` 或 OSS server-side copy。
- `Stream`：通过 `open_reader` + `open_writer` 由客户端流式复制。
- `Mixed`：树复制中部分条目使用服务端复制，部分条目降级为 stream copy。
- `diagnostics` 保存 provider-specific 统计，例如 request count、multipart count、server-side copy id。

`FileSystem::copy` 建议返回 `FsResult<CopyOutcome>`，而不是只返回 `CopyStats`。这样上层可以知道复制是如何完成的，并做审计或性能分析。

### 23.9 与 `rs-local-files` 的迁移关系

现有 `rs-local-files`：

```rust
pub struct CopyDirOptions {
    pub overwrite: bool,
    pub follow_symlinks: bool,
    pub preserve_permissions: bool,
}

pub struct CopyDirStats {
    pub files: u64,
    pub directories: u64,
    pub bytes: u64,
}
```

迁移建议：

- `CopyDirOptions.overwrite` 映射到 `CopyOptions.conflict=Overwrite` 或 `Fail`。
- `CopyDirOptions.follow_symlinks` 映射到 `CopyOptions.follow_symlinks`。
- `CopyDirOptions.preserve_permissions` 映射到 `MetadataPreservePolicy::ProviderNative` 或更窄的本地 metadata policy。
- `CopyDirStats` 映射到 `CopyStats`，未涉及字段填 0。
- 后续可以把本地类型改名为 `LocalCopyDirOptions` / `LocalCopyDirStats`，或在 `qubit-fs-local` 中做适配。

`qubit-fs` 不应依赖 `qubit-local-files`。依赖方向仍然是 `qubit-fs-local` 同时依赖 `qubit-fs` 和 `qubit-local-files`。

- `server_side=Required` 时，如果无法服务端复制，必须返回 `UnsupportedOperation`。
- 跨 provider 复制不放进基础 `FileSystem::copy`；由上层基于 `FileSystemRegistry::resource()` 或 `FsOperations` 做 stream copy。

## 24. 目录流与分页

`list` 不应直接返回 `Vec<DirEntry>` 作为底层唯一接口，因为远端后端可能分页且目录很大。

建议核心 trait：

```rust
pub trait DirectoryStream: std::fmt::Debug + Send {
    fn next_entry(&mut self) -> FsResult<Option<DirEntry>>;
}
```

便利方法可以在扩展 trait 中提供：

```rust
pub trait DirectoryStreamExt {
    fn collect_entries(self: Box<Self>) -> FsResult<Vec<DirEntry>>;
}
```

这样可以同时支持：

- 本地文件系统的 `read_dir` 迭代。
- OSS/S3 的 continuation token 分页。
- HDFS 的批量 listStatus。
- FTP 的一次性 LIST 返回。

`DirEntry` 应至少包含：

```rust
pub struct DirEntry {
    pub path: FsPath,
    pub name: String,
    pub kind: FileKind,
    pub metadata: Option<FileMetadata>,
}
```

`metadata=None` 表示本次 list 没有请求或没有拿到完整 metadata，不等同于文件没有 metadata。

## 25. URI 解析与 provider 选择规则

URI 解析建议按以下顺序执行：

1. 用 `url` crate 解析完整 URI。
2. scheme 小写规范化，并作为 provider selector 的默认值。
3. 根据 scheme 构造 `FsAuthority`。
4. URL path 解码策略由 `PathDecodePolicy` 控制。
5. query 中允许的非敏感参数进入 `Metadata`。
6. 明文敏感参数默认拒绝，除非调用者显式开启 unsafe parse policy。
7. 通过 `FileSystemRegistry` 查找 provider。
8. provider 根据 `FileSystemConfig` 创建 `Arc<dyn FileSystem>`。

provider selector 与 URI scheme 的关系：

- 默认 selector 等于 scheme。
- provider descriptor 可以声明 alias，例如 `local` provider 的 alias 包含 `file`。
- `FileSystemRegistry::fs(&FsUri)` 先按 scheme 匹配 provider alias。
- 如果 URI query 或外部配置明确指定 provider selector，则必须校验该 provider 是否声明支持对应 scheme。

不要允许 `provider=local` 打开 `oss://bucket/key` 这类语义错配，除非这是后续明确设计的 adapter 场景。

## 26. 凭据与安全边界

`rs-fs` 需要把路径、配置、凭据严格分开：

- `FsUri` 可以包含定位信息，但不应持久保存 secret。
- `FileSystemConfig` 可以包含 `CredentialRef`。
- `CredentialRef` 表示凭据来源，不表示凭据明文。
- 后端 provider 负责把 `CredentialRef` 解析成 SDK 所需凭据。
- debug / display 默认不得输出 secret。

建议 `CredentialRef` 初始形状：

```rust
pub enum CredentialRef {
    DefaultChain,
    Profile(String),
    Environment { access_key: String, secret_key: String },
    Provider(String),
}
```

注意这里的 `Environment { access_key, secret_key }` 是环境变量名，不是环境变量值。

敏感信息处理原则：

- URI 中的 password、token、access_key、secret_key 默认视为不安全输入。
- 错误信息和日志只记录 provider id、operation、path 摘要，不记录 query 中敏感字段。
- 如果后端 SDK 的错误包含敏感字段，provider 需要先清洗再放进 `FsError.message`；原始 source 可保留，但不应在 Display 中展开。

## 27. 与工具 crate 的潜在改造点

当前 SPI 已支持运行时注册和选择、创建两阶段错误。如果实现过程中发现新的能力缺口，可以按以下方向继续演进。

### 27.1 `qubit-spi`

可能需要的增强：

- `ProviderDescriptor` 增加 typed attributes，用于声明支持的 URI scheme、capability tags。
- `ProviderDescriptor` 增加 typed attributes 后，可让领域门面在创建前筛选 provider。
- 为 registry 增加显式注销或替换语义，但必须先定义并发快照与现有消费者的可见性。

不建议的改造：

- 不要把 `qubit-spi` 改成强制全局自动注册。
- 不要让 `qubit-spi` 知道文件系统、URI、credential 等领域概念。

### 27.2 `qubit-io`

可能需要的增强：

- 增加 `Close` / `Commit` 类生命周期 trait 的讨论，但不一定放入 `qubit-io`。
- 增加异步 I/O wrapper 时，应该在独立 async crate 中处理，避免污染同步 API。

### 27.3 `qubit-local-files`

可能需要的增强：

- 暴露更细的 atomic write option，例如 fsync 文件、fsync 父目录、临时文件前缀。
- 提供 root sandbox path resolver，统一处理目录穿越和 symlink policy。
- 增加本地 metadata 到统一 `FileMetadata` 的转换 helper，但该 helper 应放在 `qubit-fs-local` 更合适。
- 将现有 `TempFile` / `TempDir` 重命名为 `LocalTempFile` / `LocalTempDir`，或者先在 `qubit-fs-local` 中以新名称适配导出。

### 27.4 `qubit-metadata`

可能需要的增强：

- 增加常用 metadata key 的 schema helper。
- 增加敏感字段标记能力，但不要把 secret value 放进普通 `Metadata`。

### 27.5 `qubit-atomic`

可能需要的增强：

- 如果 registry/cache 热更新成为核心需求，可复用 `ArcAtomicRef`。
- 不应为了 `rs-fs` 把 `qubit-atomic` 改成文件系统专用。

## 28. 测试与验收计划

虽然当前阶段只写设计文档，后续实现时应按以下测试层次验收。

### 28.1 核心类型测试

- `FsPath` 解析、join、parent、file_name。
- absolute / relative path 行为。
- `.`、`..`、重复 `/`、尾随 `/`。
- URL path decode 策略。
- 非法 scheme、非法 authority、空 path。
- Windows-like path 输入在非 local provider 下的处理。

### 28.2 SPI 与 registry 测试

- provider id 与 alias 注册。
- scheme 到 provider alias 的解析。
- unknown provider 错误映射。
- provider unavailable 与 create failed 的错误区分。
- fallback 顺序与优先级。
- provider descriptor 冲突。

### 28.3 本地 provider 测试

- root sandbox 不能逃逸。
- read/write/list/delete/rename/copy 基本行为。
- atomic replace 不产生半写文件。
- `LocalTempFile` / `LocalTempDir` 能适配抽象层 `TempFile` / `TempDir`。
- `Drop` 尽力清理 不 panic。
- `cleanup()` 能返回真实删除错误。
- `persist()` 后不会再次清理目标路径。
- `keep()` 后不会在 drop 时删除资源。
- symlink 删除策略。
- metadata 映射。
- `exists` 不吞权限错误。

### 28.4 临时资源通用测试

`ManagedTempFile` / `ManagedTempDir` 至少应覆盖：

- 临时路径生成不冲突。
- `cleanup()` 调用底层 `delete`，并传递错误。
- `Drop` 对未持久化资源执行 尽力清理。
- `persist()` 成功后解除清理责任。
- `keep()` 成功后解除清理责任并返回路径。
- rename atomic required 不支持时返回 `UnsupportedOperation`。
- 允许降级时 `persist()` 可用 copy + delete 完成。
- `ManagedTempDir` 对不支持 temp dir 的后端返回明确错误。

### 28.5 复制模型通用测试

`CopyOptions` / `CopyStats` / `CopyOutcome` 至少应覆盖：

- `CopyMode::File` 遇到目录或 prefix 时返回明确错误。
- `CopyMode::Tree` 能统计文件、目录、对象、prefix 和字节数。
- `CopyConflictPolicy::Fail` 遇到已有目标时停止。
- `CopyConflictPolicy::Overwrite` 正确增加 `overwritten`。
- `CopyConflictPolicy::Skip` 正确增加 `skipped`。
- `ServerSidePreference::Require` 在后端不支持时返回 `UnsupportedOperation`。
- `ServerSidePreference::Disable` 强制使用 stream copy。
- `MetadataPreservePolicy` 能区分 portable、user metadata 和 provider-native metadata。
- `continue_on_error=true` 时失败条目计入 `failed` 并继续处理。
- `rs-local-files` 的 `CopyDirStats` 子集可以无损映射到 `CopyStats`。

### 28.6 WebDAV provider 合约测试

`qubit-fs-webdev` 至少应覆盖：

- `PROPFIND Depth: 0` 到 `metadata` 的映射。
- `PROPFIND Depth: 1` 到非递归 `list` 的映射。
- `GET` 到 reader 的映射。
- `PUT` 到 writer commit 的映射。
- `MKCOL` 创建目录。
- `DELETE` 删除资源。
- `MOVE` 的 rename 语义是尽力执行。
- `COPY` 的 copy 语义是尽力执行。
- HTTP 401/403/404/405/409/412/423/507 到统一错误类型的映射。
- ETag 到 `FileMetadata.etag` 的映射。

### 28.7 对象存储 provider 合约测试

真实 OSS/S3 provider 可以先用 fake client 做契约测试：

- prefix list 分页。
- multipart writer commit / abort。
- copy + delete rename 降级时不声明 atomic。
- conditional write 失败映射到 `PreconditionFailed`。
- object metadata 与 user metadata 区分。

### 28.8 文档与示例测试

- README 中只展示稳定 API。
- 设计文档中的代码块在实现后转为 rustdoc 示例或测试片段。
- 中文文档后缀使用 `.zh_CN.md`。

## 29. 实现顺序建议

后续真正开始编码时，建议按以下顺序：

1. 只实现 `FsPath`、`FsUri`、错误类型和 metadata 类型。
2. 用 mock provider 跑通 `qubit-spi` registry。
3. 定义同步 `FileSystem` trait 和 option 类型。
4. 实现 `CopyOptions`、`CopyStats`、`CopyOutcome` 以及复制策略类型。
5. 实现 `TempFile`、`TempDir` 抽象接口和 `ManagedTempFile`、`ManagedTempDir` 默认实现。
6. 实现 `qubit-fs-local`，先覆盖本地最小能力。
7. 实现 `qubit-fs-webdev` 的 fake-server 合约测试与最小 HTTP/WebDAV provider。
8. 根据 Local + WebDAV 两种层级语义反推核心 trait 是否过窄或过宽。
9. 再接入 OSS fake client，验证对象存储语义是否能表达。
10. 最后再决定是否需要 async trait 和自动发现 feature。

不建议一开始就同时实现 FTP、OSS、HDFS。先用 Local + WebDAV + fake object store 验证抽象是否足够，再扩真实后端。

## 30. 关键设计结论

- `std::path::Path` 只能用于本地后端内部，不能作为统一跨后端路径类型。
- 核心应区分 `FsUri` 与 `FsPath`：前者用于解析 provider，后者用于文件系统实例内操作。
- 不定义 `FsKind` 固定枚举，provider id 和 URI scheme 由 `qubit-spi` 管理。
- `FileSystem` trait 应保持对象安全，避免泛型方法污染核心接口。
- 写入接口需要显式 `commit` / `abort`，否则远端 multipart/object write 很难表达正确生命周期。
- `TempResource` 应抽取临时资源共有生命周期能力，`TempFile` / `TempDir` 继续保留文件和目录的类型语义。
- `TempFile` / `TempDir` 应表示拥有清理责任的临时资源句柄，保留 `Drop` 尽力清理、显式 `cleanup()`、`persist()` 和 `keep()` 语义。
- `ManagedTempFile` / `ManagedTempDir` 可以由核心层基于 `Arc<dyn FileSystem>` 和 `FsPath` 实现，后端只在需要更强语义时覆盖。
- `CopyOptions` / `CopyStats` / `CopyOutcome` 应成为 `rs-fs` 的通用复制模型；`rs-local-files` 的 `CopyDirOptions` / `CopyDirStats` 后续可映射或重命名为本地专用类型。
- 能力差异必须显式建模，不要让所有后端假装支持 POSIX。
- `qubit-spi`、`qubit-io`、`qubit-local-files`、`qubit-metadata` 当前已经足够支撑 MVP 方案；`qubit-atomic` 可作为实现细节 feature，而不是核心 API 前提。
