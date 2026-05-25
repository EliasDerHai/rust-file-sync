use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContentHash(u32);

impl ContentHash {
    pub fn unknown() -> Self {
        ContentHash(0)
    }

    pub fn as_u32(self) -> u32 {
        self.0
    }
}

impl From<u32> for ContentHash {
    fn from(v: u32) -> Self {
        ContentHash(v)
    }
}

impl From<ContentHash> for i64 {
    fn from(h: ContentHash) -> i64 {
        h.0 as i64
    }
}

impl From<i64> for ContentHash {
    fn from(v: i64) -> ContentHash {
        ContentHash(v as u32)
    }
}

impl From<&Path> for ContentHash {
    fn from(value: &Path) -> Self {
        crc32_of_file(value)
    }
}

impl From<&PathBuf> for ContentHash {
    fn from(value: &PathBuf) -> Self {
        crc32_of_file(value)
    }
}

fn crc32_of_file(path: &Path) -> ContentHash {
    let Ok(file) = File::open(path) else {
        return ContentHash::unknown();
    };
    let mut reader = BufReader::new(file);
    let mut hasher = Hasher::new();
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => hasher.update(&buf[..n]),
            Err(e) => {
                tracing::warn!("crc32_of_file: read error for {:?}: {e}", path);
                return ContentHash::unknown();
            }
        }
    }
    ContentHash::from(hasher.finalize())
}
