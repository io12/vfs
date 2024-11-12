use vfs::{Fs, StandaloneFs};

#[test]
fn test() {
    let mut fs = vfs_http::HttpsFs::new();
    let url = "example.com";
    assert_eq!(
        fs.metadata(url).unwrap(),
        vfs::Metadata {
            file_type: vfs::FileType::File,
            len: 1256
        }
    );
    assert!(std::io::read_to_string(fs.open(url).unwrap())
        .unwrap()
        .contains("Example Domain"));
}
