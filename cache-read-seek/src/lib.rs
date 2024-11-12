use std::{
    collections::{hash_map::Entry, HashMap},
    io::{Read, Seek, SeekFrom},
};

const SIZE: u64 = 0x1000;

pub struct CachedReadSeek<R: Read + Seek> {
    reader: R,
    cache: HashMap<u64, [u8; SIZE as usize]>,
}

impl<R: Read + Seek> CachedReadSeek<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            cache: HashMap::new(),
        }
    }
}

impl<R: Read + Seek> Read for CachedReadSeek<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let start = self.stream_position()?;
        let total_size = self.reader.seek(SeekFrom::End(0))?;
        let delta = start % SIZE;
        let cache_start = start - delta;
        let cache_read_size = match total_size.checked_sub(cache_start) {
            Some(x) => SIZE.min(x) as usize,
            None => {
                self.seek(SeekFrom::Start(start))?;
                return Ok(0);
            }
        };
        let entry = self.cache.entry(cache_start);
        let data = match entry {
            Entry::Occupied(ref data) => data.get(),
            Entry::Vacant(entry) => {
                let mut data = [0; SIZE as usize];
                self.reader.seek(SeekFrom::Start(cache_start))?;
                self.reader.read_exact(&mut data[..cache_read_size])?;
                entry.insert(data)
            }
        };
        let delta = delta as usize;
        let n = buf
            .len()
            .min((total_size - start) as usize)
            .min(SIZE as usize - delta);
        buf[..n].copy_from_slice(&data[delta..delta + n]);
        self.reader.seek(SeekFrom::Start(start + n as u64))?;
        Ok(n)
    }
}

impl<R: Read + Seek> Seek for CachedReadSeek<R> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.reader.seek(pos)
    }
}
