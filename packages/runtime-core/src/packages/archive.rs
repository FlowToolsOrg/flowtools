//! Strict, bounded ZIP admission over the same bytes the DSSE verifier checked.
//! Produces inert data, never paths, installed identities, grants or executable code.
use super::{
    hash, valid_path, verify_blob, BlobDescriptor, PackageError, VerifiedPackageDescriptor,
};
use std::{
    collections::BTreeMap,
    io::{Cursor, Read},
};

const MAX_EXPANDED_BYTES: u64 = 67_108_864 + 1_048_576;
const MAX_ENTRIES: usize = 2049;

/// Only this module can create the byte inventory. Its metadata remains a
/// descriptor receipt, not proof of Manifest compatibility or installed trust.
pub struct ValidatedArchive {
    descriptor: VerifiedPackageDescriptor,
    files: BTreeMap<String, Vec<u8>>,
}
impl ValidatedArchive {
    pub fn descriptor(&self) -> &VerifiedPackageDescriptor {
        &self.descriptor
    }
    pub fn files(&self) -> &BTreeMap<String, Vec<u8>> {
        &self.files
    }
}

struct Entry {
    name: String,
    size: u64,
    compressed: u64,
    method: u16,
    offset: usize,
    data: usize,
    directory: bool,
}
fn invalid<T>() -> Result<T, PackageError> {
    Err(PackageError::ArchiveInvalid)
}
fn slice(bytes: &[u8], start: usize, length: usize) -> Result<&[u8], PackageError> {
    bytes
        .get(
            start
                ..start
                    .checked_add(length)
                    .ok_or(PackageError::ArchiveInvalid)?,
        )
        .ok_or(PackageError::ArchiveInvalid)
}
fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, PackageError> {
    Ok(u16::from_le_bytes(
        slice(bytes, offset, 2)?.try_into().unwrap(),
    ))
}
fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, PackageError> {
    Ok(u32::from_le_bytes(
        slice(bytes, offset, 4)?.try_into().unwrap(),
    ))
}

/// v1 install profile: ZIP32, stored/deflate, no encryption, extra fields,
/// comments, data descriptors, prefixes, trailing bytes or split volumes.
/// Central and local headers must agree exactly; every local byte is accounted
/// for, preventing ambiguous names, overlapping records and hidden entries.
fn preflight(bytes: &[u8]) -> Result<Vec<Entry>, PackageError> {
    let end = bytes
        .len()
        .checked_sub(22)
        .ok_or(PackageError::ArchiveInvalid)?;
    if slice(bytes, end, 4)? != b"PK\x05\x06"
        || u16_at(bytes, end + 4)? != 0
        || u16_at(bytes, end + 6)? != 0
        || u16_at(bytes, end + 8)? != u16_at(bytes, end + 10)?
        || u16_at(bytes, end + 20)? != 0
    {
        return invalid();
    }
    let count = u16_at(bytes, end + 10)? as usize;
    let central_size = u32_at(bytes, end + 12)? as usize;
    let central_start = u32_at(bytes, end + 16)? as usize;
    if count == 0 || count > MAX_ENTRIES {
        return Err(PackageError::ArchiveBudgetExceeded);
    }
    if central_start.checked_add(central_size) != Some(end) {
        return invalid();
    }
    let mut cursor = central_start;
    let mut entries = Vec::with_capacity(count);
    let mut names = BTreeMap::new();
    let mut total = 0_u64;
    for _ in 0..count {
        if slice(bytes, cursor, 4)? != b"PK\x01\x02" {
            return invalid();
        }
        let made_by = u16_at(bytes, cursor + 4)? >> 8;
        let needed = u16_at(bytes, cursor + 6)?;
        let flags = u16_at(bytes, cursor + 8)?;
        let method = u16_at(bytes, cursor + 10)?;
        let crc = u32_at(bytes, cursor + 16)?;
        let compressed = u32_at(bytes, cursor + 20)? as u64;
        let size = u32_at(bytes, cursor + 24)? as u64;
        let name_len = u16_at(bytes, cursor + 28)? as usize;
        let attributes = u32_at(bytes, cursor + 38)?;
        let offset = u32_at(bytes, cursor + 42)? as usize;
        if !matches!(made_by, 0 | 3)
            || needed > 20
            || flags & !0x800 != 0
            || !matches!(method, 0 | 8)
            || u16_at(bytes, cursor + 30)? != 0
            || u16_at(bytes, cursor + 32)? != 0
            || u16_at(bytes, cursor + 34)? != 0
            || attributes & 0xffff & !0x31 != 0
        {
            return invalid();
        }
        let name_bytes = slice(bytes, cursor + 46, name_len)?;
        let name = std::str::from_utf8(name_bytes)
            .map_err(|_| PackageError::ArchiveInvalid)?
            .to_owned();
        let directory = name.ends_with('/');
        let path = if directory {
            &name[..name.len() - 1]
        } else {
            &name
        };
        let mode = (attributes >> 16) & 0xf000;
        if !valid_path(path)
            || names.insert(path.to_owned(), directory).is_some()
            || (mode != 0 && mode != if directory { 0x4000 } else { 0x8000 })
            || (attributes & 0x10 != 0 && !directory)
            || (directory && (size != 0 || compressed != 0 || method != 0 || crc != 0))
            || (method == 0 && size != compressed)
        {
            return invalid();
        }
        total = total
            .checked_add(size)
            .ok_or(PackageError::ArchiveBudgetExceeded)?;
        if total > MAX_EXPANDED_BYTES
            || size > 67_108_864
            || (size > 1_048_576 && size > compressed.saturating_mul(1000))
        {
            return Err(PackageError::ArchiveBudgetExceeded);
        }
        if slice(bytes, offset, 4)? != b"PK\x03\x04"
            || u16_at(bytes, offset + 4)? != needed
            || u16_at(bytes, offset + 6)? != flags
            || u16_at(bytes, offset + 8)? != method
            || slice(bytes, offset + 10, 4)? != slice(bytes, cursor + 12, 4)?
            || u32_at(bytes, offset + 14)? != crc
            || u32_at(bytes, offset + 18)? as u64 != compressed
            || u32_at(bytes, offset + 22)? as u64 != size
            || u16_at(bytes, offset + 26)? as usize != name_len
            || u16_at(bytes, offset + 28)? != 0
            || slice(bytes, offset + 30, name_len)? != name_bytes
        {
            return invalid();
        }
        let data = offset
            .checked_add(30 + name_len)
            .ok_or(PackageError::ArchiveInvalid)?;
        if data
            .checked_add(compressed as usize)
            .is_none_or(|end| end > central_start)
        {
            return invalid();
        }
        entries.push(Entry {
            name,
            size,
            compressed,
            method,
            offset,
            data,
            directory,
        });
        cursor = cursor
            .checked_add(46 + name_len)
            .ok_or(PackageError::ArchiveInvalid)?;
        if cursor > end {
            return invalid();
        }
    }
    if cursor != end {
        return invalid();
    }
    for name in names.keys() {
        let mut path = name.as_str();
        while let Some((parent, _)) = path.rsplit_once('/') {
            if names.get(parent) == Some(&false) {
                return invalid();
            }
            path = parent;
        }
    }
    let mut sorted: Vec<_> = entries.iter().collect();
    sorted.sort_by_key(|entry| entry.offset);
    let mut expected = 0;
    for entry in sorted {
        if entry.offset != expected {
            return invalid();
        }
        expected = entry.data + entry.compressed as usize;
    }
    if expected != central_start {
        return invalid();
    }
    Ok(entries)
}

pub fn validate_archive(
    descriptor: &VerifiedPackageDescriptor,
    manifest: &[u8],
    archive: &[u8],
) -> Result<ValidatedArchive, PackageError> {
    let payload = descriptor.payload();
    verify_blob(manifest, &payload.manifest, 1_048_576)?;
    verify_blob(
        archive,
        &BlobDescriptor {
            sha256: payload.archive.sha256.clone(),
            size: payload.archive.size,
        },
        67_108_864,
    )?;
    let entries = preflight(archive)?;
    let mut expected: BTreeMap<_, _> = payload
        .files
        .iter()
        .map(|file| (file.path.as_str(), (file.size, file.sha256.clone())))
        .collect();
    expected.insert("manifest.json", (manifest.len() as u64, hash(manifest)));
    if entries.iter().filter(|entry| !entry.directory).count() != expected.len() {
        return invalid();
    }
    // Validate the complete inventory and size budget before decompression.
    for entry in &entries {
        if entry.directory {
            if !expected.keys().any(|file| file.starts_with(&entry.name)) {
                return invalid();
            }
        } else if expected
            .get(entry.name.as_str())
            .is_none_or(|(size, _)| *size != entry.size)
        {
            return invalid();
        }
    }
    let mut zip =
        zip::ZipArchive::new(Cursor::new(archive)).map_err(|_| PackageError::ArchiveInvalid)?;
    if zip.len() != entries.len() {
        return invalid();
    }
    let mut files = BTreeMap::new();
    for (index, entry) in entries.iter().enumerate() {
        let mut file = zip
            .by_index(index)
            .map_err(|_| PackageError::ArchiveInvalid)?;
        if file.name().map_err(|_| PackageError::ArchiveInvalid)? != entry.name
            || file.size() != entry.size
            || file.compressed_size() != entry.compressed
            || file.header_start() != entry.offset as u64
            || file.data_start() != Some(entry.data as u64)
            || file.compression()
                != if entry.method == 0 {
                    zip::CompressionMethod::Stored
                } else {
                    zip::CompressionMethod::Deflated
                }
        {
            return invalid();
        }
        if entry.directory {
            continue;
        }
        if entry.method == 8 {
            // ZIP readers may ignore unused bytes after a deflate end marker.
            // Require one complete stream consuming the entire signed span.
            let mut decoder = flate2::Decompress::new(false);
            let mut output = vec![0; entry.size as usize + 1];
            let status = decoder
                .decompress(
                    slice(archive, entry.data, entry.compressed as usize)?,
                    &mut output,
                    flate2::FlushDecompress::Finish,
                )
                .map_err(|_| PackageError::ArchiveInvalid)?;
            if status != flate2::Status::StreamEnd
                || decoder.total_in() != entry.compressed
                || decoder.total_out() != entry.size
            {
                return invalid();
            }
        }
        let mut bytes = Vec::new();
        (&mut file)
            .take(entry.size + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| PackageError::ArchiveInvalid)?;
        if bytes.len() as u64 != entry.size || hash(&bytes) != expected[entry.name.as_str()].1 {
            return Err(PackageError::IntegrityMismatch);
        }
        files.insert(entry.name.clone(), bytes);
    }
    if files
        .get("manifest.json")
        .is_none_or(|bytes| bytes != manifest)
    {
        return invalid();
    }
    Ok(ValidatedArchive {
        descriptor: descriptor.clone(),
        files,
    })
}
