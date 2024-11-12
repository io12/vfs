use std::{
    io::{Read, Seek, SeekFrom},
    num::ParseIntError,
};

use cache_read_seek::CachedReadSeek;
use reqwest::{
    blocking::{Client, RequestBuilder},
    header::{HeaderValue, ToStrError, CONTENT_LENGTH, RANGE},
    StatusCode,
};

pub struct HttpFs(Client);

pub struct HttpsFs(Client);

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("failed reading content length")]
    ContentLength(#[from] ContentLengthError),
    #[error("failed sending HTTP request")]
    HttpRequest(#[from] reqwest::Error),
}

#[derive(thiserror::Error, Debug)]
pub enum ContentLengthError {
    #[error(transparent)]
    SendRequest(#[from] reqwest::Error),
    #[error(transparent)]
    ToStr(#[from] ToStrError),
    #[error(transparent)]
    ParseInt(#[from] ParseIntError),
}

fn content_length(client: &Client, url: &str) -> Result<u64, ContentLengthError> {
    Ok(client.get(url).send()?.headers()[CONTENT_LENGTH]
        .to_str()?
        .parse()?)
}

fn path_to_url(use_https: bool, path: &str) -> String {
    let protocol = if use_https { "https" } else { "http" };
    format!("{protocol}://{path}")
}

fn metadata(client: &Client, use_https: bool, path: &str) -> Result<vfs::Metadata, Error> {
    let url = path_to_url(use_https, path);
    Ok(vfs::Metadata {
        len: content_length(client, &url)?,
        file_type: vfs::FileType::File,
    })
}

fn open(client: &Client, use_https: bool, path: &str) -> Result<HttpFile, Error> {
    let url = path_to_url(use_https, path);
    Ok(HttpFile(CachedReadSeek::new(CachelessHttpFile {
        size: content_length(client, &url)?,
        offset: 0,
        request: client.get(url),
    })))
}

macro_rules! impl_fs {
    ($T:ty) => {
        impl vfs::Fs for $T {
            type Path = str;
            type Error = Error;
            type File = HttpFile;

            fn metadata(&mut self, path: &str) -> Result<vfs::Metadata, Error> {
                metadata(&self.0, false, path)
            }

            fn open(&mut self, path: &str) -> Result<Self::File, Error> {
                open(&self.0, false, path)
            }
        }

        impl vfs::StandaloneFs for $T {
            fn new() -> Self {
                Self(Client::new())
            }
        }
    };
}

impl_fs!(HttpFs);
impl_fs!(HttpsFs);

struct CachelessHttpFile {
    size: u64,
    offset: u64,
    request: RequestBuilder,
}

impl Seek for CachelessHttpFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let add_err = || {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid seek to a negative or overflowing position",
            )
        };
        self.offset = match pos {
            SeekFrom::Start(offset) => Ok(offset),
            SeekFrom::End(offset) => self.size.checked_add_signed(offset).ok_or_else(add_err),
            SeekFrom::Current(offset) => self.offset.checked_add_signed(offset).ok_or_else(add_err),
        }?;
        Ok(self.offset)
    }
}

impl Read for CachelessHttpFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let range = HeaderValue::from_str(&format!(
            "bytes={}-{}",
            self.offset,
            self.offset + buf.len() as u64
        ))
        .expect("Invalid range HTTP header value");
        let mut response = self
            .request
            .try_clone()
            .unwrap()
            .header(RANGE, range)
            .send()
            .unwrap();
        if response.status() == StatusCode::RANGE_NOT_SATISFIABLE {
            return Ok(0);
        }
        let n = response.read(buf)?;
        self.offset += n as u64;
        Ok(n)
    }
}

pub struct HttpFile(CachedReadSeek<CachelessHttpFile>);

impl Seek for HttpFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

impl Read for HttpFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}
