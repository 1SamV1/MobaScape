use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("cache parse error: {0}")]
    Parse(String),

    #[error("unexpected end of buffer at offset {0}")]
    Eof(usize),

    #[error("index {index} has no group {group}")]
    GroupOutOfRange { index: u32, group: u32 },

    #[error("unsupported compression type {0}")]
    UnsupportedCompression(u8),

    #[error("xtea failure: {0}")]
    Xtea(String),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("missing file {0}")]
    MissingFile(PathBuf),

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn parse(msg: impl Into<String>) -> Self {
        Self::Parse(msg.into())
    }
}
