use vfs::{FileType, Fs, Metadata};
use vfs_meta::MetaFs;

#[test]
fn test() {
    assert_eq!(
        MetaFs
            .metadata(
                [
                    "http://www.unforgettable.dk/42.zip",
                    "libarchive:lib 0.zip",
                    "libarchive:book 0.zip",
                    "libarchive:chapter 0.zip",
                    "libarchive:doc 0.zip",
                    "libarchive:page 0.zip",
                    "libarchive:0.dll"
                ]
                .join("|")
            )
            .unwrap(),
        Metadata {
            file_type: FileType::File,
            len: 4294967295
        }
    );
}
