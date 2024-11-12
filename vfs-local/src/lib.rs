pub struct LocalFs;

impl vfs::StandaloneFs for LocalFs {
    fn new() -> Self {
        Self
    }
}

impl vfs::Fs for LocalFs {
    type Path = std::path::Path;
    type Error = std::io::Error;
    type File = std::fs::File;

    fn metadata(&mut self, path: &Self::Path) -> Result<vfs::Metadata, Self::Error> {
        let m = std::fs::metadata(path)?;
        let file_type = m.file_type();
        let file_type = if file_type.is_file() {
            vfs::FileType::File
        } else if file_type.is_dir() {
            vfs::FileType::Dir
        } else if file_type.is_symlink() {
            vfs::FileType::SymLink
        } else {
            todo!()
        };
        Ok(vfs::Metadata {
            file_type,
            len: m.len(),
        })
    }

    fn open(&mut self, path: &Self::Path) -> Result<Self::File, Self::Error> {
        std::fs::File::open(path)
    }
}
