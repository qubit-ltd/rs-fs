use qubit_fs::FsPath;

#[test]
fn test_root_path_reports_root_properties() {
    let root = FsPath::root();

    assert!(root.is_absolute());
    assert_eq!("/", root.as_str());
    assert_eq!(None, root.parent());
    assert_eq!(None, root.file_name());
    assert_eq!(None, root.file_extension());
    assert_eq!("/", root.to_string());
}

#[test]
fn test_parse_normalizes_segments_and_rejects_invalid_paths() {
    let path = FsPath::parse("/a//b/./c").expect("path should parse");

    assert_eq!("/a/b/c", path.as_str());
    assert_eq!(Some("c"), path.file_name());
    assert!(FsPath::parse("").is_err());
    assert!(FsPath::parse("../escape").is_err());
    assert!(FsPath::parse("bad\0path").is_err());
    assert_eq!(
        "/",
        FsPath::parse("/a/..")
            .expect("absolute parent normalization should reach root")
            .as_str(),
    );
    assert_eq!(
        "b",
        FsPath::parse("a/../b")
            .expect("relative parent normalization should succeed")
            .as_str(),
    );
    assert!(FsPath::parse("a/..").is_err());
}

#[test]
fn test_parent_and_join_handle_root_absolute_and_relative_paths() {
    let root = FsPath::root();
    let path = FsPath::parse("/a//b/./c").expect("path should parse");

    assert_eq!("/a/b", path.parent().expect("path should have parent").as_str());
    assert_eq!(
        "/a/b/d",
        path.parent()
            .expect("parent should exist")
            .join("d")
            .expect("join should succeed")
            .as_str(),
    );
    assert_eq!(
        "/absolute",
        path.join("/absolute")
            .expect("absolute child should replace base")
            .as_str(),
    );
    assert_eq!("/child", root.join("child").expect("root join should succeed").as_str(),);
    assert_eq!(
        "/",
        FsPath::parse("/a")
            .expect("path should parse")
            .parent()
            .expect("absolute child should have root parent")
            .as_str(),
    );
    assert_eq!(
        "a",
        FsPath::parse("a/b")
            .expect("relative path should parse")
            .parent()
            .expect("relative nested path should have parent")
            .as_str(),
    );
    assert_eq!(
        None,
        FsPath::parse("file")
            .expect("single relative path should parse")
            .parent(),
    );
}

#[test]
fn test_file_extension_returns_last_meaningful_extension() {
    assert_eq!(
        Some("gz"),
        FsPath::parse("/a/archive.tar.gz")
            .expect("path should parse")
            .file_extension(),
    );
    assert_eq!(
        None,
        FsPath::parse("/a/.profile")
            .expect("path should parse")
            .file_extension(),
    );
    assert_eq!(
        None,
        FsPath::parse("/a/trailing.")
            .expect("path should parse")
            .file_extension(),
    );
}
