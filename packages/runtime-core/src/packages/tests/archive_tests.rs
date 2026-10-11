use super::*;
use crate::packages::archive::validate_archive;
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

fn zip_bytes(files: &[(&str, &[u8])], method: CompressionMethod) -> Vec<u8> {
    let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in files {
        let options = SimpleFileOptions::default().compression_method(method);
        if name.ends_with('/') {
            zip.add_directory(*name, options).unwrap();
        } else {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
    }
    zip.finish().unwrap().into_inner()
}
fn fixture(method: CompressionMethod) -> Fixture {
    let mut fixture = Fixture::new();
    let source = b"throw new Error('FIXTURE_MUST_NOT_EXECUTE')";
    fixture.package.files = vec![PackageFile {
        path: "commands.js".into(),
        size: source.len() as u64,
        sha256: hash(source),
    }];
    let mut manifest: Value = serde_json::from_slice(&fixture.manifest).unwrap();
    manifest["files"] = serde_json::to_value(&fixture.package.files).unwrap();
    fixture.manifest = serde_json::to_vec(&manifest).unwrap();
    fixture.package.manifest = BlobDescriptor {
        size: fixture.manifest.len() as u64,
        sha256: hash(&fixture.manifest),
    };
    fixture.archive = zip_bytes(
        &[
            ("manifest.json", &fixture.manifest),
            ("commands.js", source),
        ],
        method,
    );
    refresh(&mut fixture);
    fixture
}
fn refresh(fixture: &mut Fixture) {
    fixture.package.archive.size = fixture.archive.len() as u64;
    fixture.package.archive.sha256 = hash(&fixture.archive);
}
fn check(fixture: &Fixture) -> Result<archive::ValidatedArchive, PackageError> {
    let descriptor = fixture.verify()?;
    validate_archive(&descriptor, &fixture.manifest, &fixture.archive)
}
fn offset(bytes: &[u8], magic: &[u8], index: usize) -> usize {
    bytes
        .windows(magic.len())
        .enumerate()
        .filter(|(_, bytes)| *bytes == magic)
        .nth(index)
        .unwrap()
        .0
}
fn word(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}
fn dword(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

#[test]
fn archive_stored_deflated_and_node_golden_are_inert_exact_byte_inventories() {
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let fixture = fixture(method);
        let validated = check(&fixture).unwrap();
        assert_eq!(validated.files().len(), 2);
        assert_eq!(validated.files()["manifest.json"], fixture.manifest);
        assert!(validated.files()["commands.js"].starts_with(b"throw new Error"));
        assert_eq!(
            validated.descriptor().package_digest(),
            fixture.package.archive.sha256
        );
    }
    let golden = Fixture::new();
    let validated = check(&golden).unwrap();
    assert_eq!(validated.files().len(), golden.package.files.len() + 1);
}

#[test]
fn archive_receipt_cannot_be_reused_with_different_bytes() {
    let mut fixture = fixture(CompressionMethod::Stored);
    let receipt = fixture.verify().unwrap();
    fixture.archive[0] ^= 1;
    assert!(matches!(
        validate_archive(&receipt, &fixture.manifest, &fixture.archive),
        Err(PackageError::IntegrityMismatch)
    ));
    fixture.archive[0] ^= 1;
    fixture.manifest.push(b' ');
    assert!(matches!(
        validate_archive(&receipt, &fixture.manifest, &fixture.archive),
        Err(PackageError::IntegrityMismatch)
    ));
}

#[test]
fn archive_valid_signature_does_not_allow_missing_extra_or_substituted_files() {
    for variant in 0..5 {
        let mut fixture = fixture(CompressionMethod::Stored);
        let source = b"throw new Error('FIXTURE_MUST_NOT_EXECUTE')";
        let mut files: Vec<(&str, &[u8])> = vec![
            ("manifest.json", &fixture.manifest),
            ("commands.js", source),
        ];
        match variant {
            0 => {
                files.pop();
            }
            1 => files.push(("extra.js", b"unexpected")),
            2 => files[1].1 = b"short",
            3 => files[1].1 = b"throw new Error('FIXTURE_WILL_EXECUTE_NOW')",
            _ => files[0].1 = b"{}",
        }
        fixture.archive = zip_bytes(&files, CompressionMethod::Stored);
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(check(&fixture).is_err(), "variant {variant}");
    }
}

#[test]
fn archive_rejects_signed_local_central_header_disagreement() {
    for relative in [4, 6, 8, 10, 14, 18, 22, 26, 28, 30] {
        let mut fixture = fixture(CompressionMethod::Stored);
        let local = offset(&fixture.archive, b"PK\x03\x04", 1);
        fixture.archive[local + relative] ^= 1;
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(
            matches!(check(&fixture), Err(PackageError::ArchiveInvalid)),
            "offset {relative}"
        );
    }
}

#[test]
fn archive_rejects_signed_zip_slip_devices_ads_and_nonportable_names() {
    for name in [
        "../evil.js",
        "/evil.js",
        "c:/evil.js",
        "a\\evil.js",
        "con",
        "nul.txt",
        "file:stream",
        "a//b",
        "a/./b",
        "File.js",
        "file.",
        "file ",
        "a/../b",
        "é.js",
    ] {
        let mut fixture = fixture(CompressionMethod::Stored);
        fixture.archive = zip_bytes(
            &[("manifest.json", &fixture.manifest), (name, b"evil")],
            CompressionMethod::Stored,
        );
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(
            matches!(check(&fixture), Err(PackageError::ArchiveInvalid)),
            "{name}"
        );
    }
}

#[test]
fn archive_rejects_signed_symlink_special_files_and_windows_reparse_attributes() {
    for attributes in [
        0xa1ff0000_u32,
        0x11ff0000,
        0x61ff0000,
        0x81ff0400,
        0x81ff0008,
        0x81ff0010,
    ] {
        let mut fixture = fixture(CompressionMethod::Stored);
        let central = offset(&fixture.archive, b"PK\x01\x02", 1);
        dword(&mut fixture.archive, central + 38, attributes);
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(
            matches!(check(&fixture), Err(PackageError::ArchiveInvalid)),
            "{attributes:x}"
        );
    }
}

#[test]
fn archive_rejects_encryption_descriptors_zip64_unknown_algorithms_and_extra_fields() {
    for (relative, value) in [(6, 45), (8, 1), (8, 8), (10, 99), (30, 1), (32, 1), (34, 1)] {
        let mut fixture = fixture(CompressionMethod::Stored);
        let central = offset(&fixture.archive, b"PK\x01\x02", 0);
        word(&mut fixture.archive, central + relative, value);
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(
            matches!(check(&fixture), Err(PackageError::ArchiveInvalid)),
            "{relative}:{value}"
        );
    }
}

#[test]
fn archive_rejects_split_volumes_comments_truncation_prefixes_and_trailing_bytes() {
    for variant in 0..7 {
        let mut fixture = fixture(CompressionMethod::Stored);
        let end = fixture.archive.len() - 22;
        match variant {
            0 => word(&mut fixture.archive, end + 4, 1),
            1 => word(&mut fixture.archive, end + 6, 1),
            2 => word(&mut fixture.archive, end + 8, 1),
            3 => word(&mut fixture.archive, end + 20, 1),
            4 => {
                fixture.archive.pop();
            }
            5 => fixture.archive.insert(0, 0),
            _ => fixture.archive.push(0),
        }
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(
            matches!(check(&fixture), Err(PackageError::ArchiveInvalid)),
            "variant {variant}"
        );
    }
}

#[test]
fn archive_rejects_overlap_hidden_local_records_and_duplicate_names() {
    for variant in 0..3 {
        let mut fixture = fixture(CompressionMethod::Stored);
        let central = offset(&fixture.archive, b"PK\x01\x02", 1);
        match variant {
            0 => dword(&mut fixture.archive, central + 42, 0),
            1 => {
                // Remove one central entry but retain its hidden local record.
                let end = fixture.archive.len() - 22;
                let start = offset(&fixture.archive, b"PK\x01\x02", 0);
                let size = (central - start) as u32;
                word(&mut fixture.archive, end + 8, 1);
                word(&mut fixture.archive, end + 10, 1);
                dword(&mut fixture.archive, end + 12, size);
                fixture.archive.drain(central..end);
            }
            _ => {
                let local = offset(&fixture.archive, b"PK\x03\x04", 1);
                fixture.archive[local + 30..local + 41].copy_from_slice(b"manifest.js");
                fixture.archive[central + 46..central + 57].copy_from_slice(b"manifest.js");
            }
        }
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(check(&fixture).is_err(), "variant {variant}");
    }
}

#[test]
fn archive_crc_is_checked_even_when_local_and_central_agree() {
    let mut fixture = fixture(CompressionMethod::Stored);
    let local = offset(&fixture.archive, b"PK\x03\x04", 1);
    let central = offset(&fixture.archive, b"PK\x01\x02", 1);
    dword(&mut fixture.archive, local + 14, 0);
    dword(&mut fixture.archive, central + 16, 0);
    refresh(&mut fixture);
    fixture.verify().unwrap();
    assert!(matches!(check(&fixture), Err(PackageError::ArchiveInvalid)));
}

#[test]
fn archive_expansion_and_entry_budgets_fail_before_decompression() {
    let mut fixture = fixture(CompressionMethod::Deflated);
    let central = offset(&fixture.archive, b"PK\x01\x02", 1);
    let local = offset(&fixture.archive, b"PK\x03\x04", 1);
    dword(&mut fixture.archive, central + 24, 67_108_865);
    dword(&mut fixture.archive, local + 22, 67_108_865);
    refresh(&mut fixture);
    fixture.verify().unwrap();
    assert!(matches!(
        check(&fixture),
        Err(PackageError::ArchiveBudgetExceeded)
    ));
    let mut fixture = super::archive_tests::fixture(CompressionMethod::Stored);
    let end = fixture.archive.len() - 22;
    word(&mut fixture.archive, end + 8, 2050);
    word(&mut fixture.archive, end + 10, 2050);
    refresh(&mut fixture);
    fixture.verify().unwrap();
    assert!(matches!(
        check(&fixture),
        Err(PackageError::ArchiveBudgetExceeded)
    ));
}

#[test]
fn archive_accepts_only_necessary_directories_and_rejects_file_parent_collisions() {
    for variant in 0..3 {
        let mut fixture = fixture(CompressionMethod::Stored);
        let source = b"throw new Error('FIXTURE_MUST_NOT_EXECUTE')";
        let entries: Vec<(&str, &[u8])> = match variant {
            0 => vec![
                ("manifest.json", &fixture.manifest),
                ("commands.js/", b""),
                ("commands.js", source),
            ],
            1 => vec![
                ("manifest.json", &fixture.manifest),
                ("unused/", b""),
                ("commands.js", source),
            ],
            _ => vec![
                ("manifest.json", &fixture.manifest),
                ("commands.js", source),
                ("commands.js/a", b"evil"),
            ],
        };
        fixture.archive = zip_bytes(&entries, CompressionMethod::Stored);
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(
            matches!(check(&fixture), Err(PackageError::ArchiveInvalid)),
            "variant {variant}"
        );
    }
}

#[test]
fn archive_accepts_nested_inventory_with_necessary_directory() {
    let mut fixture = fixture(CompressionMethod::Stored);
    let source = b"throw new Error('FIXTURE_MUST_NOT_EXECUTE')";
    fixture.package.files[0].path = "executor/commands.js".into();
    let mut manifest: Value = serde_json::from_slice(&fixture.manifest).unwrap();
    manifest["files"] = serde_json::to_value(&fixture.package.files).unwrap();
    fixture.manifest = serde_json::to_vec(&manifest).unwrap();
    fixture.package.manifest = BlobDescriptor {
        size: fixture.manifest.len() as u64,
        sha256: hash(&fixture.manifest),
    };
    fixture.archive = zip_bytes(
        &[
            ("manifest.json", &fixture.manifest),
            ("executor/", b""),
            ("executor/commands.js", source),
        ],
        CompressionMethod::Stored,
    );
    refresh(&mut fixture);
    assert_eq!(
        check(&fixture).unwrap().files()["executor/commands.js"],
        source
    );
}

#[test]
fn archive_rejects_duplicate_physical_entries_with_matching_headers() {
    let mut fixture = fixture(CompressionMethod::Stored);
    fixture.archive = zip_bytes(
        &[
            ("manifest.json", &fixture.manifest),
            ("commands.js", b"source"),
            ("attackxx.js", b"evil"),
        ],
        CompressionMethod::Stored,
    );
    let local = offset(&fixture.archive, b"PK\x03\x04", 2);
    let central = offset(&fixture.archive, b"PK\x01\x02", 2);
    fixture.archive[local + 30..local + 41].copy_from_slice(b"commands.js");
    fixture.archive[central + 46..central + 57].copy_from_slice(b"commands.js");
    refresh(&mut fixture);
    fixture.verify().unwrap();
    assert!(matches!(check(&fixture), Err(PackageError::ArchiveInvalid)));
}

#[test]
fn archive_rejects_bytes_hidden_after_deflate_end_marker() {
    let mut fixture = fixture(CompressionMethod::Deflated);
    let local = offset(&fixture.archive, b"PK\x03\x04", 1);
    let central = offset(&fixture.archive, b"PK\x01\x02", 1);
    let start = offset(&fixture.archive, b"PK\x01\x02", 0);
    let end = fixture.archive.len() - 22;
    let compressed =
        u32::from_le_bytes(fixture.archive[local + 18..local + 22].try_into().unwrap());
    dword(&mut fixture.archive, local + 18, compressed + 1);
    dword(&mut fixture.archive, central + 20, compressed + 1);
    dword(&mut fixture.archive, end + 16, (start + 1) as u32);
    fixture.archive.insert(start, 0);
    refresh(&mut fixture);
    fixture.verify().unwrap();
    assert!(matches!(check(&fixture), Err(PackageError::ArchiveInvalid)));
}

#[test]
fn archive_rejects_actual_high_ratio_deflate_bomb_before_inflation() {
    let mut fixture = fixture(CompressionMethod::Deflated);
    let source = vec![0; 4_194_304];
    fixture.package.files[0].size = source.len() as u64;
    fixture.package.files[0].sha256 = hash(&source);
    let mut manifest: Value = serde_json::from_slice(&fixture.manifest).unwrap();
    manifest["files"] = serde_json::to_value(&fixture.package.files).unwrap();
    fixture.manifest = serde_json::to_vec(&manifest).unwrap();
    fixture.package.manifest = BlobDescriptor {
        size: fixture.manifest.len() as u64,
        sha256: hash(&fixture.manifest),
    };
    fixture.archive = zip_bytes(
        &[
            ("manifest.json", &fixture.manifest),
            ("commands.js", &source),
        ],
        CompressionMethod::Deflated,
    );
    refresh(&mut fixture);
    fixture.verify().unwrap();
    assert!(fixture.archive.len() < 5000);
    assert!(matches!(
        check(&fixture),
        Err(PackageError::ArchiveBudgetExceeded)
    ));
}

#[test]
fn archive_truncated_and_arbitrary_signed_inputs_never_panic() {
    let original = fixture(CompressionMethod::Stored);
    for length in (1..original.archive.len()).step_by(13) {
        let mut fixture = fixture(CompressionMethod::Stored);
        fixture.archive.truncate(length);
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(check(&fixture).is_err());
    }
    for value in [0, 0xff, b'P', b'K'] {
        let mut fixture = fixture(CompressionMethod::Stored);
        fixture.archive = vec![value; 64];
        refresh(&mut fixture);
        fixture.verify().unwrap();
        assert!(check(&fixture).is_err());
    }
}
