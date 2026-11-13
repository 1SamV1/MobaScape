use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};

use memmap2::Mmap;

use crate::error::{Error, Result};
use crate::group::{self, Group};
use crate::keys::XteaKeys;
use crate::reference::IndexData;

const SECTOR_SIZE: usize = 520;
const SECTOR_DATA_SIZE: usize = 512;
const INDEX_ENTRY_SIZE: usize = 6;
const REFERENCE_INDEX: usize = 255;

/// Read-only view of an Old School RuneScape cache directory.
pub struct Cache {
    data: Mmap,
    indices: Vec<Vec<u8>>,
    keys: Option<XteaKeys>,
    references: std::sync::Mutex<HashMap<u32, IndexData>>,
    dir: PathBuf,
}

impl std::fmt::Debug for Cache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cache")
            .field("dir", &self.dir)
            .field("data_len", &self.data.len())
            .field("keys", &self.keys.as_ref().map(XteaKeys::len))
            .finish()
    }
}

impl Cache {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        Self::open_with_keys(dir, None::<&Path>)
    }

    pub fn open_with_keys(
        dir: impl AsRef<Path>,
        keys_path: Option<impl AsRef<Path>>,
    ) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        let data_path = dir.join("main_file_cache.dat2");
        if !data_path.exists() {
            return Err(Error::MissingFile(data_path));
        }

        let file = File::open(&data_path)?;
        // The cache is a read-only mmap; the file is never mutated by this process.
        let data = unsafe { Mmap::map(&file)? };

        let mut indices = Vec::with_capacity(REFERENCE_INDEX + 1);
        for i in 0..=REFERENCE_INDEX {
            let path = dir.join(format!("main_file_cache.idx{i}"));
            if path.exists() {
                indices.push(std::fs::read(path)?);
            } else {
                indices.push(Vec::new());
            }
        }

        let keys = match keys_path {
            Some(path) => Some(XteaKeys::from_json(path)?),
            None => {
                let path = dir.join("keys.json");
                if path.exists() {
                    Some(XteaKeys::from_json(path)?)
                } else {
                    None
                }
            }
        };

        Ok(Self {
            data,
            indices,
            keys,
            references: std::sync::Mutex::new(HashMap::new()),
            dir,
        })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn keys(&self) -> Option<&XteaKeys> {
        self.keys.as_ref()
    }

    pub fn group_count(&self, index: u32) -> u32 {
        self.indices
            .get(index as usize)
            .map(|i| (i.len() / INDEX_ENTRY_SIZE) as u32)
            .unwrap_or(0)
    }

    pub fn index_len(&self, index: u32) -> usize {
        self.indices.get(index as usize).map(Vec::len).unwrap_or(0)
    }

    fn entry(&self, index: u32, group: u32) -> Result<(usize, usize)> {
        let idx = self
            .indices
            .get(index as usize)
            .ok_or(Error::GroupOutOfRange { index, group })?;
        let off = group as usize * INDEX_ENTRY_SIZE;
        let entry = idx
            .get(off..off + INDEX_ENTRY_SIZE)
            .ok_or(Error::GroupOutOfRange { index, group })?;
        let length = ((entry[0] as usize) << 16) | ((entry[1] as usize) << 8) | entry[2] as usize;
        let sector = ((entry[3] as usize) << 16) | ((entry[4] as usize) << 8) | entry[5] as usize;
        Ok((length, sector))
    }

    /// Read the raw sector chain for a group, truncated to the index length.
    pub fn raw_group(&self, index: u32, group: u32) -> Result<Vec<u8>> {
        let (length, mut sector) = self.entry(index, group)?;
        if length == 0 {
            return Ok(Vec::new());
        }

        let mut out = Vec::with_capacity(length);
        let mut visited = 0usize;

        while out.len() < length {
            if sector == 0 {
                break;
            }
            let off = sector
                .checked_mul(SECTOR_SIZE)
                .ok_or_else(|| Error::parse("sector offset overflow"))?;
            let header = self
                .data
                .get(off..off + 8)
                .ok_or_else(|| Error::parse(format!("sector {sector} out of range")))?;
            let chunk = self
                .data
                .get(off + 8..off + SECTOR_SIZE)
                .ok_or_else(|| Error::parse(format!("sector {sector} data out of range")))?;

            out.extend_from_slice(chunk);
            sector =
                ((header[4] as usize) << 16) | ((header[5] as usize) << 8) | header[6] as usize;
            visited += 1;
            if visited > 1_000_000 {
                return Err(Error::parse("sector chain loop"));
            }
        }

        out.truncate(length);
        if out.len() != length {
            return Err(Error::parse(format!(
                "group {index}/{group}: expected {length} bytes, got {}",
                out.len()
            )));
        }
        Ok(out)
    }

    pub fn group(&self, index: u32, group: u32) -> Result<Group> {
        let raw = self.raw_group(index, group)?;
        let key = self.keys.as_ref().and_then(|k| k.get(index, group));
        group::decode(&raw, key)
    }

    pub fn reference(&self, index: u32) -> Result<IndexData> {
        if let Some(cached) = self
            .references
            .lock()
            .expect("reference cache poisoned")
            .get(&index)
        {
            return Ok(cached.clone());
        }

        let group = self.group(REFERENCE_INDEX as u32, index)?;
        let parsed = IndexData::parse(&group.data)?;
        self.references
            .lock()
            .expect("reference cache poisoned")
            .insert(index, parsed.clone());
        Ok(parsed)
    }

    /// Split a decompressed archive group into its files.
    pub fn archive_files(&self, index: u32, archive: u32) -> Result<Vec<Vec<u8>>> {
        let group = self.group(index, archive)?;
        let reference = self.reference(index)?;
        let meta = reference.archive(archive);

        let file_count = meta.map(|a| a.files.len()).unwrap_or(1);
        if file_count <= 1 {
            return Ok(vec![group.data]);
        }

        split_archive(&group.data, file_count)
    }

    pub fn file(&self, index: u32, archive: u32, file_id: u32) -> Result<Vec<u8>> {
        let files = self.archive_files(index, archive)?;
        let reference = self.reference(index)?;
        let meta = reference
            .archive(archive)
            .ok_or_else(|| Error::parse(format!("missing archive {index}/{archive}")))?;
        let pos = meta
            .files
            .iter()
            .position(|f| f.id == file_id)
            .ok_or_else(|| Error::parse(format!("missing file {index}/{archive}/{file_id}")))?;
        files
            .into_iter()
            .nth(pos)
            .ok_or_else(|| Error::parse("archive file index out of range"))
    }

    pub fn config(&self, archive: u32, file_id: u32) -> Result<Vec<u8>> {
        self.file(2, archive, file_id)
    }
}

fn split_archive(data: &[u8], file_count: usize) -> Result<Vec<Vec<u8>>> {
    if data.is_empty() {
        return Err(Error::parse("empty multi-file archive"));
    }

    let chunks = *data.last().unwrap() as usize;
    if chunks == 0 {
        return Err(Error::parse("archive has zero chunks"));
    }

    let table_len = chunks
        .checked_mul(file_count)
        .and_then(|v| v.checked_mul(4))
        .ok_or_else(|| Error::parse("archive table overflow"))?;
    if data.len() < table_len + 1 {
        return Err(Error::parse("archive chunk table out of range"));
    }

    let table_start = data.len() - 1 - table_len;
    let mut chunk_sizes = vec![vec![0usize; chunks]; file_count];
    let mut file_sizes = vec![0usize; file_count];

    let mut pos = table_start;
    for chunk in 0..chunks {
        let mut chunk_size: i64 = 0;
        for id in 0..file_count {
            let delta =
                i32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as i64;
            pos += 4;
            chunk_size += delta;
            if chunk_size < 0 {
                return Err(Error::parse("negative archive chunk size"));
            }
            let chunk_size = chunk_size as usize;
            chunk_sizes[id][chunk] = chunk_size;
            file_sizes[id] = file_sizes[id].saturating_add(chunk_size);
        }
    }

    let mut files = file_sizes
        .iter()
        .map(|&size| Vec::with_capacity(size))
        .collect::<Vec<_>>();
    let mut read_pos = 0usize;

    for chunk in 0..chunks {
        for (id, file) in files.iter_mut().enumerate() {
            let size = chunk_sizes[id][chunk];
            let end = read_pos
                .checked_add(size)
                .ok_or_else(|| Error::parse("archive read overflow"))?;
            let bytes = data
                .get(read_pos..end)
                .ok_or_else(|| Error::parse("archive file data out of range"))?;
            file.extend_from_slice(bytes);
            read_pos = end;
        }
    }

    Ok(files)
}
