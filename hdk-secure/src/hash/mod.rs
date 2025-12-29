use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AfsHash(pub i32);

impl AfsHash {
    pub fn new_from_str(s: &str) -> Self {
        Self(afs_hash(s.chars()))
    }

    pub fn new_from_path(path: &Path) -> Self {
        let s = path.to_str().unwrap_or_default();
        Self(afs_hash(s.chars()))
    }
}

impl core::fmt::Display for AfsHash {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:08X}", self.0 as u32)
    }
}

pub fn afs_hash(data: std::str::Chars) -> i32 {
    let mut hash: i32 = 0;

    for mut c in data {
        if c == '\\' {
            c = '/';
        }
        c = c.to_lowercase().next().unwrap();

        hash = hash.overflowing_mul(0x25).0; // 37
        hash += c as i32;
    }

    hash
}
