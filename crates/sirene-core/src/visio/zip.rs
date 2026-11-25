//! A reader for the ZIP archives that hold OPC packages (VISIO-2): stored and deflate entries, no
//! zip64, encryption, or multiple disks.

use std::collections::HashMap;

use super::limits;
use crate::{Error, Result};

const EOCD_SIG: u32 = 0x0605_4b50;
const CENTRAL_SIG: u32 = 0x0201_4b50;
const LOCAL_SIG: u32 = 0x0403_4b50;
const EOCD_LEN: usize = 22;
const CENTRAL_LEN: usize = 46;
const LOCAL_LEN: usize = 30;

#[derive(Debug, Clone)]
struct Entry {
    method: u16,
    crc: u32,
    compressed: usize,
    size: usize,
    offset: usize,
}

/// An opened archive: the entries named by its central directory, read on demand.
#[derive(Debug)]
pub(crate) struct Archive<'a> {
    data: &'a [u8],
    /// Entries by lowercased name (OPC part names compare case-insensitively).
    entries: HashMap<String, Entry>,
    /// Bytes inflated so far, against [`limits::MAX_TOTAL`].
    inflated: usize,
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    let end = at.checked_add(2)?;
    Some(u16::from_le_bytes(d.get(at..end)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    let end = at.checked_add(4)?;
    Some(u32::from_le_bytes(d.get(at..end)?.try_into().ok()?))
}

fn truncated() -> Error {
    Error::new("the archive is truncated or damaged")
}

/// Whether `name` is a safe, relative, `/`-separated part name: no empty or `..` segments, no
/// backslash, NUL, or drive prefix (VISIO-2).
pub(crate) fn safe_name(name: &str) -> bool {
    let name = name.strip_suffix('/').unwrap_or(name);
    if name.is_empty() || name.starts_with('/') || name.contains(['\\', '\0']) {
        return false;
    }
    if name.as_bytes().get(1) == Some(&b':') {
        return false;
    }
    name.split('/').all(|s| !s.is_empty() && s != "..")
}

impl<'a> Archive<'a> {
    pub(crate) fn open(data: &'a [u8]) -> Result<Self> {
        if data.len() > limits::MAX_ARCHIVE {
            return Err(Error::new("the archive is over 64 MiB"));
        }
        if data.get(..4).and_then(|s| <[u8; 4]>::try_from(s).ok()) != Some(*b"PK\x03\x04") {
            return Err(Error::new("not a ZIP archive (no PK header)"));
        }
        let eocd = find_eocd(data)?;
        let disk = u16_at(data, eocd + 4).ok_or_else(truncated)?;
        let cd_disk = u16_at(data, eocd + 6).ok_or_else(truncated)?;
        let count = usize::from(u16_at(data, eocd + 10).ok_or_else(truncated)?);
        let cd_size = u32_at(data, eocd + 12).ok_or_else(truncated)?;
        let cd_offset = u32_at(data, eocd + 16).ok_or_else(truncated)?;
        if disk != 0 || cd_disk != 0 {
            return Err(Error::new("multi-disk archives are not supported"));
        }
        if count == 0xFFFF || cd_size == u32::MAX || cd_offset == u32::MAX {
            return Err(Error::new("zip64 archives are not supported"));
        }
        if count > limits::MAX_ENTRIES {
            return Err(Error::new(format!(
                "the archive has {count} entries, over {}",
                limits::MAX_ENTRIES
            )));
        }
        let mut at = usize::try_from(cd_offset).map_err(|_| truncated())?;
        let mut entries = HashMap::new();
        let mut total = 0usize;
        for _ in 0..count {
            if u32_at(data, at) != Some(CENTRAL_SIG) {
                return Err(Error::new("the archive's central directory is damaged"));
            }
            let flags = u16_at(data, at + 8).ok_or_else(truncated)?;
            let method = u16_at(data, at + 10).ok_or_else(truncated)?;
            let crc = u32_at(data, at + 16).ok_or_else(truncated)?;
            let compressed = u32_at(data, at + 20).ok_or_else(truncated)?;
            let size = u32_at(data, at + 24).ok_or_else(truncated)?;
            let name_len = usize::from(u16_at(data, at + 28).ok_or_else(truncated)?);
            let extra_len = usize::from(u16_at(data, at + 30).ok_or_else(truncated)?);
            let comment_len = usize::from(u16_at(data, at + 32).ok_or_else(truncated)?);
            let offset = u32_at(data, at + 42).ok_or_else(truncated)?;
            let name_at = at + CENTRAL_LEN;
            let name_end = name_at.checked_add(name_len).ok_or_else(truncated)?;
            let raw = data.get(name_at..name_end).ok_or_else(truncated)?;
            let name = String::from_utf8_lossy(raw).into_owned();
            at = name_end
                .checked_add(extra_len)
                .and_then(|a| a.checked_add(comment_len))
                .ok_or_else(truncated)?;
            if flags & 1 != 0 {
                return Err(Error::new("encrypted archives are not supported"));
            }
            if compressed == u32::MAX || size == u32::MAX || offset == u32::MAX {
                return Err(Error::new("zip64 archives are not supported"));
            }
            if !safe_name(&name) {
                return Err(Error::new(format!("unsafe entry name {name:?}")));
            }
            if name.ends_with('/') {
                continue;
            }
            let (compressed, size) = (to_usize(compressed)?, to_usize(size)?);
            if size > limits::MAX_PART {
                return Err(Error::new(format!("entry {name:?} is over 32 MiB")));
            }
            if method == 0 && compressed != size {
                return Err(Error::new(format!("entry {name:?} has inconsistent sizes")));
            }
            if size > limits::RATIO_FLOOR && size / limits::MAX_RATIO > compressed {
                return Err(Error::new(format!(
                    "entry {name:?} expands more than {} to 1",
                    limits::MAX_RATIO
                )));
            }
            total = total.saturating_add(size);
            if total > limits::MAX_TOTAL {
                return Err(Error::new("the archive expands to over 128 MiB"));
            }
            let key = name.to_ascii_lowercase();
            let entry = Entry {
                method,
                crc,
                compressed,
                size,
                offset: to_usize(offset)?,
            };
            if entries.insert(key, entry).is_some() {
                return Err(Error::new(format!("duplicate entry name {name:?}")));
            }
        }
        Ok(Archive {
            data,
            entries,
            inflated: 0,
        })
    }

    /// Whether the archive has an entry called `name`, in any case.
    pub(crate) fn contains(&self, name: &str) -> bool {
        self.entries.contains_key(&name.to_ascii_lowercase())
    }

    /// The entry names, lowercased, in no particular order.
    #[cfg(test)]
    pub(crate) fn names(&self) -> Vec<String> {
        let mut names: Vec<_> = self.entries.keys().cloned().collect();
        names.sort();
        names
    }

    /// The bytes of the entry called `name` (in any case), checked against its CRC-32.
    pub(crate) fn read(&mut self, name: &str) -> Result<Vec<u8>> {
        let entry = self
            .entries
            .get(&name.to_ascii_lowercase())
            .ok_or_else(|| Error::new(format!("the package has no part {name:?}")))?
            .clone();
        if entry.method != 0 && entry.method != 8 {
            return Err(Error::new(format!(
                "entry {name:?} uses unsupported compression method {}",
                entry.method
            )));
        }
        let at = entry.offset;
        if u32_at(self.data, at) != Some(LOCAL_SIG) {
            return Err(Error::new(format!("entry {name:?} has a damaged header")));
        }
        let name_len = usize::from(u16_at(self.data, at + 26).ok_or_else(truncated)?);
        let extra_len = usize::from(u16_at(self.data, at + 28).ok_or_else(truncated)?);
        let start = at
            .checked_add(LOCAL_LEN + name_len + extra_len)
            .ok_or_else(truncated)?;
        let end = start.checked_add(entry.compressed).ok_or_else(truncated)?;
        let raw = self.data.get(start..end).ok_or_else(truncated)?;
        self.inflated = self.inflated.saturating_add(entry.size);
        if self.inflated > limits::MAX_TOTAL {
            return Err(Error::new("the parts read expand to over 128 MiB"));
        }
        let bytes = if entry.method == 0 {
            raw.to_vec()
        } else {
            miniz_oxide::inflate::decompress_to_vec_with_limit(raw, entry.size)
                .map_err(|_| Error::new(format!("entry {name:?} does not inflate")))?
        };
        if bytes.len() != entry.size {
            return Err(Error::new(format!("entry {name:?} has the wrong size")));
        }
        if crc32(&bytes) != entry.crc {
            return Err(Error::new(format!("entry {name:?} fails its checksum")));
        }
        Ok(bytes)
    }
}

fn to_usize(n: u32) -> Result<usize> {
    usize::try_from(n).map_err(|_| truncated())
}

/// The offset of the end-of-central-directory record: the last signature in the final 64 KiB.
fn find_eocd(data: &[u8]) -> Result<usize> {
    let last = data.len().checked_sub(EOCD_LEN).ok_or_else(truncated)?;
    let floor = last.saturating_sub(usize::from(u16::MAX));
    (floor..=last)
        .rev()
        .find(|&at| u32_at(data, at) == Some(EOCD_SIG))
        .ok_or_else(|| Error::new("not a ZIP archive (no end of central directory)"))
}

/// The CRC-32 (IEEE) of `data`.
pub(crate) fn crc32(data: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &b in data {
        crc ^= u32::from(b);
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

#[cfg(test)]
mod tests;
