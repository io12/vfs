use std::io::{Read, Seek};

#[derive(Debug, PartialEq, Eq)]
pub enum FileType {
    Dir,
    File,
    SymLink,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Metadata {
    pub file_type: FileType,
    pub len: u64,
}

pub trait Fs {
    type Path: ?Sized;
    type Error;
    type File: Read;

    fn metadata(&mut self, path: &Self::Path) -> Result<Metadata, Self::Error>;

    fn open(&mut self, path: &Self::Path) -> Result<Self::File, Self::Error>;
}

pub trait StandaloneFs: Fs {
    fn new() -> Self;
}

pub trait IoBackedFs<R: Read + Seek>: Fs {
    type Password;

    fn from_io(io: R, password: Self::Password) -> Self;
}
