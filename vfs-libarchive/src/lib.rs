use cache_read_seek::CachedReadSeek;
use libarchive_sys::{
    archive, archive_entry, archive_entry_filetype, archive_entry_pathname, archive_entry_size,
    archive_errno, archive_error_string, archive_read_add_passphrase, archive_read_close,
    archive_read_data_block, archive_read_free, archive_read_new, archive_read_next_header,
    archive_read_open2, archive_read_set_seek_callback, archive_read_support_filter_all,
    archive_read_support_format_all, archive_set_error, ARCHIVE_EOF, ARCHIVE_OK, SEEK_CUR,
    SEEK_END, SEEK_SET,
};
use std::{
    cell::RefCell,
    ffi::{c_void, CStr, CString},
    io::{Read, Seek, SeekFrom},
    ptr::{null, null_mut},
    rc::Rc,
};
use typed_path::UnixPath;

struct ClientData<R: Read + Seek> {
    reader: R,
    buf: [u8; 4096],
}

struct InnerFs<R: Read + Seek> {
    a: *mut archive,
    client_data: *mut ClientData<R>,
    password: Option<CString>,
}

pub struct LibArchiveFs<R: Read + Seek> {
    inner: Rc<RefCell<InnerFs<R>>>,
}

unsafe extern "C" fn read_callback<R: Read + Seek>(
    a: *mut archive,
    client_data: *mut c_void,
    buf: *mut *const c_void,
) -> isize {
    let client_data = &mut *(client_data as *mut ClientData<R>);
    let n = match client_data.reader.read(&mut client_data.buf) {
        Ok(n) => n,
        Err(err) => {
            let s = CString::new(format!("failed reading from backing IO: {err}"))
                .expect("IO error string has null byte");
            archive_set_error(a, err.raw_os_error().unwrap_or_default(), s.as_ptr());
            return -1;
        }
    };
    *buf = client_data.buf.as_ptr() as *const c_void;
    n as isize
}

unsafe extern "C" fn skip_callback<R: Read + Seek>(
    _: *mut archive,
    client_data: *mut c_void,
    request: i64,
) -> i64 {
    let client_data = &mut *(client_data as *mut ClientData<R>);
    client_data.reader.seek(SeekFrom::Current(request)).unwrap();
    request
}

unsafe extern "C" fn seek_callback<R: Read + Seek>(
    _: *mut archive,
    client_data: *mut c_void,
    offset: i64,
    whence: i32,
) -> i64 {
    let client_data = &mut *(client_data as *mut ClientData<R>);
    let pos = match whence {
        SEEK_SET => SeekFrom::Start(offset.try_into().unwrap()),
        SEEK_CUR => SeekFrom::Current(offset),
        SEEK_END => SeekFrom::End(offset),
        _ => unreachable!(),
    };
    client_data.reader.seek(pos).unwrap().try_into().unwrap()
}

impl<R: Read + Seek> InnerFs<R> {
    unsafe fn init_archive(&mut self) {
        self.a = archive_read_new();
        if let Some(password) = &self.password {
            assert_eq!(
                archive_read_add_passphrase(self.a, password.as_ptr()),
                ARCHIVE_OK
            );
        }
        assert_eq!(archive_read_support_format_all(self.a), ARCHIVE_OK);
        assert_eq!(archive_read_support_filter_all(self.a), ARCHIVE_OK);
        assert_eq!(
            archive_read_set_seek_callback(self.a, Some(seek_callback::<R>)),
            ARCHIVE_OK
        );
        assert_eq!(
            archive_read_open2(
                self.a,
                self.client_data as *mut c_void,
                None,
                Some(read_callback::<R>),
                Some(skip_callback::<R>),
                None,
            ),
            ARCHIVE_OK
        );
    }

    unsafe fn free_archive(&mut self) {
        assert_eq!(archive_read_close(self.a), ARCHIVE_OK);
        assert_eq!(archive_read_free(self.a), ARCHIVE_OK);
    }

    unsafe fn rewind(&mut self) {
        self.free_archive();
        (*self.client_data).reader.rewind().unwrap();
        self.init_archive();
    }

    unsafe fn find_entry(&mut self, path: &[u8]) -> *mut archive_entry {
        let path = UnixPath::new(path);
        unsafe {
            self.rewind();
            let mut entry = null_mut();
            loop {
                let r = archive_read_next_header(self.a, &mut entry);
                match r {
                    ARCHIVE_OK => {
                        let entry_path = archive_entry_pathname(entry);
                        let entry_path = CStr::from_ptr(entry_path).to_bytes();
                        let entry_path = typed_path::UnixPath::new(entry_path);
                        dbg!(entry_path);
                        if entry_path == path {
                            return entry;
                        }
                    }
                    ARCHIVE_EOF => todo!("entry not found: {path}"),
                    r => todo!(
                        "{r}: {} {:?}",
                        archive_errno(self.a),
                        (archive_error_string(self.a))
                    ),
                };
            }
        }
    }
}

impl<R: Read + Seek> Drop for InnerFs<R> {
    fn drop(&mut self) {
        unsafe {
            self.free_archive();
            let _ = Box::from_raw(self.client_data as *mut ClientData<R>);
        }
    }
}

struct CachelessFile<R: Read + Seek> {
    fs: LibArchiveFs<R>,
    path: Vec<u8>,
    size: u64,
    offset: u64,
}

pub struct File<R: Read + Seek>(CachedReadSeek<CachelessFile<R>>);

struct DataBlockIter<'fs, R: Read + Seek> {
    fs: &'fs mut LibArchiveFs<R>,
    prev_end: Option<i64>,
    next: Option<(i64, usize, *const u8)>,
}

impl<R: Read + Seek> Iterator for DataBlockIter<'_, R> {
    type Item = (i64, usize, Option<*const u8>);

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            if let Some((offset, size, ptr)) = self.next {
                self.next = None;
                self.prev_end = Some(offset + size as i64);
                return Some((offset, size, Some(ptr)));
            }

            let inner = self.fs.inner.borrow_mut();

            let mut block_ptr = null();
            let mut block_size = 0;
            let mut block_offset = 0;
            let r = archive_read_data_block(
                inner.a,
                &mut block_ptr,
                &mut block_size,
                &mut block_offset,
            );
            let block_ptr = block_ptr as *const u8;
            let block_end = block_offset + block_size as i64;

            match r {
                ARCHIVE_OK => {
                    if self.prev_end.is_none() || self.prev_end == Some(block_offset) {
                        // Adjacent block
                        self.prev_end = Some(block_end);
                        Some((block_offset, block_size, Some(block_ptr)))
                    } else {
                        // Gap before block
                        let prev_end = self.prev_end.unwrap();
                        assert!(prev_end < block_offset);
                        let gap_offset = prev_end;
                        let gap_size = block_offset - gap_offset;
                        self.next = Some((block_offset, block_size, block_ptr));
                        self.prev_end = Some(gap_offset + gap_size);
                        Some((gap_offset, gap_size as usize, None))
                    }
                }
                ARCHIVE_EOF => None,
                _ => panic!("{:?}", CStr::from_ptr(archive_error_string(inner.a))),
            }
        }
    }
}

impl<R: Read + Seek> CachelessFile<R> {
    fn data_blocks(&mut self) -> DataBlockIter<'_, R> {
        DataBlockIter {
            fs: &mut self.fs,
            prev_end: None,
            next: None,
        }
    }
}

impl<R: Read + Seek> Read for CachelessFile<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        unsafe {
            let offset = self.offset;
            let _ = self.fs.inner.borrow_mut().find_entry(&self.path);
            let Some((block_offset, block_size, block_ptr)) =
                self.data_blocks().find(|(block_offset, block_size, _)| {
                    let block_end = *block_offset as u64 + *block_size as u64;
                    offset < block_end
                })
            else {
                return Ok(0);
            };
            assert!(block_offset as u64 <= offset);
            let skip = offset as usize - block_offset as usize;
            let n = buf.len().min(block_size - skip);
            match block_ptr {
                // Regular data
                Some(block_ptr) => {
                    let block_data = std::slice::from_raw_parts(block_ptr, block_size);
                    buf[..n].copy_from_slice(&block_data[skip..(skip + n)]);
                }
                // Gap to fill with zeros
                None => buf[..n].fill(0),
            }
            self.seek(SeekFrom::Current(n.try_into().unwrap()))?;
            Ok(n)
        }

        // TODO
        // let mut n = 0;
        // let mut prev_block_end = None;
        // unsafe {
        //     let mut inner = self.fs.inner.borrow_mut();
        //     let _ = inner.find_entry(&self.path);
        //     let mut block_ptr = null();
        //     let mut block_size = 0;
        //     let mut block_offset = 0;
        //     while n < buf.len() {
        //         let r = archive_read_data_block(
        //             inner.a,
        //             &mut block_ptr,
        //             &mut block_size,
        //             &mut block_offset,
        //         );
        //         if r == ARCHIVE_EOF {
        //             break;
        //         }
        //         if r.is_negative() {
        //             panic!("{:?}", CStr::from_ptr(archive_error_string(inner.a)));
        //         }
        //         let block_end = block_offset as usize + block_size;
        //         if block_end as u64 <= self.offset + n as u64 {
        //             continue;
        //         }
        //         if self.offset + buf.len() as u64 <= block_offset as u64 {
        //             break;
        //         }
        //         if self.offset + (n as u64) < block_offset as u64 {
        //             let rel_offset = block_offset as u64 - self.offset - n as u64;
        //             if buf.len() as u64 <= rel_offset {
        //                 n = buf.len();
        //                 break;
        //             } else {
        //                 n = rel_offset as usize;
        //             }
        //         }
        //         let block = std::slice::from_raw_parts(block_ptr as *const u8, block_size);
        //         let buf_remaining = buf.len() - n;
        //         let copy_len = block_size.min(buf_remaining);
        //         buf[n..(n + copy_len)].copy_from_slice(&block[..copy_len]);
        //         n += copy_len;
        //     }
        // }
        // self.offset += n as u64;
        // Ok(n)
    }
}

impl<R: Read + Seek> Seek for CachelessFile<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.offset = match pos {
            SeekFrom::Start(offset) => offset,
            SeekFrom::End(offset) => self.size.checked_add_signed(offset).unwrap(),
            SeekFrom::Current(offset) => self.offset.checked_add_signed(offset).unwrap(),
        };
        Ok(self.offset)
    }
}

impl<R: Read + Seek> Read for File<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.0.read(buf)
    }
}

impl<R: Read + Seek> Seek for File<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.0.seek(pos)
    }
}

// TODO: Try deriving this
impl<R: Read + Seek> Clone for LibArchiveFs<R> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<R: Read + Seek> vfs::IoBackedFs<R> for LibArchiveFs<R> {
    type Password = Option<CString>;

    fn from_io(io: R, password: Self::Password) -> Self {
        let mut inner = InnerFs {
            a: null_mut(),
            client_data: Box::into_raw(Box::new(ClientData {
                reader: io,
                buf: [0; 4096],
            })),
            password,
        };
        unsafe { inner.init_archive() }
        Self {
            inner: Rc::new(RefCell::new(inner)),
        }
    }
}

impl<R: Read + Seek> vfs::Fs for LibArchiveFs<R> {
    type Path = [u8];
    type Error = ();
    type File = File<R>;

    fn metadata(&mut self, path: &[u8]) -> Result<vfs::Metadata, Self::Error> {
        unsafe {
            let entry = self.inner.borrow_mut().find_entry(path);
            Ok(vfs::Metadata {
                file_type: match archive_entry_filetype(entry) {
                    0o100000 => vfs::FileType::File,
                    0o120000 => vfs::FileType::SymLink,
                    0o040000 => vfs::FileType::Dir,
                    _ => todo!(),
                },
                len: archive_entry_size(entry).try_into().unwrap(),
            })
        }
    }

    fn open(&mut self, path: &[u8]) -> Result<Self::File, Self::Error> {
        let path = path.to_vec();
        let m = self.metadata(&path)?;
        assert_eq!(m.file_type, vfs::FileType::File);
        Ok(File(CachedReadSeek::new(CachelessFile {
            fs: self.clone(),
            path,
            size: m.len,
            offset: 0,
        })))
    }
}
