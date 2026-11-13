use crate::buffer::Buffer;
use crate::error::{Error, Result};

#[derive(Debug, Clone, Default)]
pub struct FileData {
    pub id: u32,
    pub name_hash: i32,
}

#[derive(Debug, Clone, Default)]
pub struct ArchiveData {
    pub id: u32,
    pub name_hash: i32,
    pub crc: i32,
    pub revision: i32,
    pub compressed_size: i32,
    pub decompressed_size: i32,
    pub files: Vec<FileData>,
}

#[derive(Debug, Clone, Default)]
pub struct IndexData {
    pub protocol: u8,
    pub revision: i32,
    pub named: bool,
    pub sized: bool,
    pub archives: Vec<ArchiveData>,
}

impl IndexData {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let mut b = Buffer::new(data);
        let protocol = b.read_u8()?;
        if !(5..=7).contains(&protocol) {
            return Err(Error::parse(format!(
                "unsupported index protocol {protocol}"
            )));
        }

        let revision = if protocol >= 6 { b.read_i32()? } else { 0 };
        let flags = b.read_u8()?;
        let named = flags & 0x01 != 0;
        let sized = flags & 0x04 != 0;
        if flags & !0x05 != 0 {
            return Err(Error::parse(format!("unknown index flags {flags:#x}")));
        }

        let archive_count = if protocol >= 7 {
            b.read_big_smart()? as usize
        } else {
            b.read_u16()? as usize
        };

        let mut archives = Vec::with_capacity(archive_count);
        let mut last_id = 0u32;
        for _ in 0..archive_count {
            let delta = if protocol >= 7 {
                b.read_big_smart()? as u32
            } else {
                b.read_u16()? as u32
            };
            last_id = last_id.wrapping_add(delta);
            archives.push(ArchiveData {
                id: last_id,
                ..Default::default()
            });
        }

        if named {
            for archive in &mut archives {
                archive.name_hash = b.read_i32()?;
            }
        }

        for archive in &mut archives {
            archive.crc = b.read_i32()?;
        }

        if sized {
            for archive in &mut archives {
                archive.compressed_size = b.read_i32()?;
                archive.decompressed_size = b.read_i32()?;
            }
        }

        for archive in &mut archives {
            archive.revision = b.read_i32()?;
        }

        let mut file_counts = Vec::with_capacity(archive_count);
        for _ in 0..archive_count {
            let count = if protocol >= 7 {
                b.read_big_smart()? as usize
            } else {
                b.read_u16()? as usize
            };
            file_counts.push(count);
        }

        for (archive, count) in archives.iter_mut().zip(file_counts) {
            let mut last_file_id = 0u32;
            for _ in 0..count {
                let delta = if protocol >= 7 {
                    b.read_big_smart()? as u32
                } else {
                    b.read_u16()? as u32
                };
                last_file_id = last_file_id.wrapping_add(delta);
                archive.files.push(FileData {
                    id: last_file_id,
                    name_hash: 0,
                });
            }
        }

        if named {
            for archive in &mut archives {
                for file in &mut archive.files {
                    file.name_hash = b.read_i32()?;
                }
            }
        }

        Ok(Self {
            protocol,
            revision,
            named,
            sized,
            archives,
        })
    }

    pub fn archive(&self, id: u32) -> Option<&ArchiveData> {
        self.archives.iter().find(|a| a.id == id)
    }
}
