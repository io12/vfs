use vfs::{Fs, IoBackedFs};

#[test]
fn test() {
    let tmp = tempfile::tempdir().unwrap();
    let tmp = tmp.path();
    let a = tmp.join("a");
    let b = tmp.join("b");
    let c = tmp.join("c");
    let d = tmp.join("d");
    let b_data = "Hello world! (B)";
    let d_data = "Hello world! (D)";
    std::fs::create_dir(&a).unwrap();
    std::fs::write(&b, b_data).unwrap();
    std::os::unix::fs::symlink("/symlink/target/path", &c).unwrap();
    std::fs::write(&d, d_data).unwrap();
    let tar = tmp.join("test.tar");
    std::process::Command::new("tar")
        .arg("cf")
        .arg(&tar)
        .arg("-C")
        .arg(tmp)
        .arg("a")
        .arg("b")
        .arg("c")
        .arg("d")
        .status()
        .unwrap();
    let mut fs = vfs_libarchive::LibArchiveFs::from_io(
        std::fs::File::open(&tar).unwrap(),
        Default::default(),
    );
    assert_eq!(
        fs.metadata(b"a").unwrap(),
        vfs::Metadata {
            file_type: vfs::FileType::Dir,
            len: 0
        }
    );
    assert_eq!(
        fs.metadata(b"b").unwrap(),
        vfs::Metadata {
            file_type: vfs::FileType::File,
            len: b_data.len() as u64,
        }
    );
    assert_eq!(
        fs.metadata(b"c").unwrap(),
        vfs::Metadata {
            file_type: vfs::FileType::SymLink,
            len: 0
        }
    );
    assert_eq!(
        fs.metadata(b"d").unwrap(),
        vfs::Metadata {
            file_type: vfs::FileType::File,
            len: d_data.len() as u64
        }
    );
    let b = fs.open(b"b").unwrap();
    let d = fs.open(b"d").unwrap();
    assert_eq!(std::io::read_to_string(b).unwrap(), b_data);
    assert_eq!(std::io::read_to_string(d).unwrap(), d_data);
}
