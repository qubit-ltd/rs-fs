use qubit_fs::Path;
use qubit_fs::read::ReadOptions;
use qubit_fs::write::WriteOptions;
use qubit_fs_local::LocalFileSystems;
use qubit_fs_local::LocalResourcePolicy;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let filesystem = LocalFileSystems::rooted(directory.path(), LocalResourcePolicy::unbounded())?;
    let path = Path::parse("/report.txt")?;
    filesystem.write_all(&path, b"report ready", WriteOptions::default())?;
    let bytes = filesystem.read_all(&path, ReadOptions::default(), 1024)?;
    assert_eq!(b"report ready", bytes.as_slice());
    println!("{}", String::from_utf8(bytes)?);
    Ok(())
}
