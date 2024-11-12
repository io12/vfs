mod parser;

use std::{
    ffi::OsStr,
    io::{Read, Seek, SeekFrom},
    os::unix::prelude::OsStrExt,
    path::{Path, PathBuf},
};
use vfs::{Fs, IoBackedFs, StandaloneFs};
use vfs_http::{HttpFs, HttpsFs};
use vfs_libarchive::LibArchiveFs;
use vfs_local::LocalFs;

pub struct MetaFs;

pub trait ReadSeek: Read + Seek {}

impl<R: Read + Seek> ReadSeek for R {}

enum AnyFs {
    Standalone(AnyStandaloneFs),
    IoBacked(AnyIoBackedFs),
}

enum AnyStandaloneFs {
    #[cfg(feature = "vfs-local")]
    Local(LocalFs),
    #[cfg(feature = "vfs-http")]
    Https(HttpsFs),
    #[cfg(feature = "vfs-http")]
    Http(HttpFs),
}

enum AnyIoBackedFs {
    #[cfg(feature = "vfs-libarchive")]
    LibArchive(LibArchiveFs<Box<dyn ReadSeek>>),
}

pub enum AnyStandaloneFile {
    #[cfg(feature = "vfs-local")]
    Local(<LocalFs as Fs>::File),
    #[cfg(feature = "vfs-http")]
    Http(<HttpFs as Fs>::File),
}

pub enum AnyIoBackedFile {
    #[cfg(feature = "vfs-libarchive")]
    LibArchive(<LibArchiveFs<Box<dyn ReadSeek>> as Fs>::File),
}

pub enum AnyFile {
    Standalone(AnyStandaloneFile),
    IoBacked(AnyIoBackedFile),
}

impl AnyStandaloneFs {
    fn from_name(name: &[u8]) -> Option<Self> {
        match name {
            #[cfg(feature = "vfs-local")]
            b"local" => Some(Self::Local(LocalFs::new())),
            #[cfg(feature = "vfs-http")]
            b"https" => Some(Self::Https(HttpsFs::new())),
            #[cfg(feature = "vfs-http")]
            b"http" => Some(Self::Http(HttpFs::new())),
            _ => None,
        }
    }
}

impl AnyIoBackedFs {
    fn from_name_io(name: &[u8], io: impl ReadSeek + 'static) -> Option<Self> {
        match name {
            #[cfg(feature = "vfs-libarchive")]
            b"libarchive" => Some(Self::LibArchive(LibArchiveFs::from_io(
                Box::new(io),
                Default::default(),
            ))),
            _ => None,
        }
    }
}

impl Read for AnyStandaloneFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(feature = "vfs-local")]
            AnyStandaloneFile::Local(x) => x.read(buf),
            #[cfg(feature = "vfs-http")]
            AnyStandaloneFile::Http(x) => x.read(buf),
        }
    }
}

impl Seek for AnyStandaloneFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            #[cfg(feature = "vfs-local")]
            AnyStandaloneFile::Local(x) => x.seek(pos),
            #[cfg(feature = "vfs-http")]
            AnyStandaloneFile::Http(x) => x.seek(pos),
        }
    }
}

impl Read for AnyIoBackedFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            #[cfg(feature = "vfs-libarchive")]
            AnyIoBackedFile::LibArchive(x) => x.read(buf),
        }
    }
}

impl Seek for AnyIoBackedFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            #[cfg(feature = "vfs-libarchive")]
            AnyIoBackedFile::LibArchive(x) => x.seek(pos),
        }
    }
}

impl Read for AnyFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match self {
            AnyFile::Standalone(x) => x.read(buf),
            AnyFile::IoBacked(x) => x.read(buf),
        }
    }
}

impl Seek for AnyFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            AnyFile::Standalone(x) => x.seek(pos),
            AnyFile::IoBacked(x) => x.seek(pos),
        }
    }
}

impl Fs for AnyFs {
    type Error = ();
    type File = AnyFile;

    fn metadata(&mut self, path: impl AsRef<Path>) -> Result<vfs::Metadata, Self::Error> {
        match self {
            AnyFs::Standalone(fs) => fs.metadata(path),
            AnyFs::IoBacked(fs) => fs.metadata(path),
        }
    }

    fn open(&mut self, path: impl AsRef<Path>) -> Result<Self::File, Self::Error> {
        match self {
            AnyFs::Standalone(fs) => fs.open(path).map(AnyFile::Standalone),
            AnyFs::IoBacked(fs) => fs.open(path).map(AnyFile::IoBacked),
        }
    }
}

impl Fs for AnyStandaloneFs {
    type Error = ();
    type File = AnyStandaloneFile;

    fn metadata(&mut self, path: impl AsRef<Path>) -> Result<vfs::Metadata, Self::Error> {
        Ok(match self {
            #[cfg(feature = "vfs-local")]
            AnyStandaloneFs::Local(x) => x.metadata(path).unwrap(),
            #[cfg(feature = "vfs-http")]
            AnyStandaloneFs::Https(x) => x.metadata(path).unwrap(),
            #[cfg(feature = "vfs-http")]
            AnyStandaloneFs::Http(x) => x.metadata(path).unwrap(),
        })
    }

    fn open(&mut self, path: impl AsRef<Path>) -> Result<Self::File, Self::Error> {
        Ok(match self {
            #[cfg(feature = "vfs-local")]
            AnyStandaloneFs::Local(x) => AnyStandaloneFile::Local(x.open(path).unwrap()),
            #[cfg(feature = "vfs-http")]
            AnyStandaloneFs::Https(x) => AnyStandaloneFile::Http(x.open(path).unwrap()),
            #[cfg(feature = "vfs-http")]
            AnyStandaloneFs::Http(x) => AnyStandaloneFile::Http(x.open(path).unwrap()),
        })
    }
}

impl Fs for AnyIoBackedFs {
    type Error = ();
    type File = AnyIoBackedFile;

    fn metadata(&mut self, path: impl AsRef<Path>) -> Result<vfs::Metadata, Self::Error> {
        match self {
            #[cfg(feature = "vfs-libarchive")]
            AnyIoBackedFs::LibArchive(x) => x.metadata(path),
        }
    }

    fn open(&mut self, path: impl AsRef<Path>) -> Result<Self::File, Self::Error> {
        match self {
            #[cfg(feature = "vfs-libarchive")]
            AnyIoBackedFs::LibArchive(x) => x.open(path).map(AnyIoBackedFile::LibArchive),
        }
    }
}

fn last_fs_and_path(path: &Path) -> (AnyFs, PathBuf) {
    let path = path.as_os_str().as_bytes();
    let meta_components = parser::parse(path).unwrap().1;
    let [(head_proto, head_path), ref tail @ ..] = meta_components.as_slice() else {
        unreachable!()
    };
    let head_fs = AnyFs::Standalone(AnyStandaloneFs::from_name(head_proto).unwrap());
    let (fs, path) = tail.into_iter().fold(
        (head_fs, head_path),
        |(mut fs, path), (tail_proto, tail_path)| {
            let path = OsStr::from_bytes(path);
            let file = fs.open(path).unwrap();
            let tail_fs = AnyFs::IoBacked(AnyIoBackedFs::from_name_io(tail_proto, file).unwrap());
            (tail_fs, tail_path)
        },
    );
    let path = Path::new(OsStr::from_bytes(path)).to_path_buf();
    (fs, path)
}

impl Fs for MetaFs {
    type Error = ();
    type File = AnyFile;

    fn metadata(&mut self, path: impl AsRef<Path>) -> Result<vfs::Metadata, Self::Error> {
        let (mut last_fs, last_path) = last_fs_and_path(path.as_ref());
        last_fs.metadata(last_path)
    }

    fn open(&mut self, path: impl AsRef<Path>) -> Result<Self::File, Self::Error> {
        let (mut last_fs, last_path) = last_fs_and_path(path.as_ref());
        last_fs.open(last_path)
    }
}
