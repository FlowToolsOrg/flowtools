use super::*;
use crate::{
    data::DataStore,
    packages::{
        tools::{
            admit_fixture, admit_tool_archive, InertToolArchive, ToolAdmissionError,
            ToolArchiveInput, ToolProgram,
        },
        trust::PackageTrustStore,
    },
};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

mod dependency_tests;
#[cfg(windows)]
mod install_tests;

struct ToolFixture {
    base: Fixture,
    manifest: Value,
    contents: BTreeMap<String, Vec<u8>>,
    method: CompressionMethod,
}
impl ToolFixture {
    fn new(method: CompressionMethod) -> Self {
        let mut base = Fixture::new();
        base.package.kind = PackageKind::Tool;
        base.package.id = "ffmpeg".into();
        base.package.source.registry = "fixture-registry".into();
        base.package.source.artifact = "fixture-ffmpeg".into();
        base.package.license = "LGPL-2.1-or-later".into();
        base.package.build_flavor = "fixture-static".into();
        let mut root = base.root.payload().clone();
        root.publishers[0].packages.push(PublisherScope {
            kind: PackageKind::Tool,
            id: "ffmpeg".into(),
            targets: vec![base.package.target.clone()],
        });
        base.golden["rootEnvelope"] =
            serde_json::from_slice(&signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2])).unwrap();
        let manifest = json!({
            "formatVersion": 1,
            "kind": "tool",
            "publisher": base.package.publisher,
            "id": base.package.id,
            "version": base.package.version,
            "target": base.package.target,
            "buildFlavor": base.package.build_flavor,
            "source": base.package.source,
            "license": base.package.license,
            "files": [],
            "programs": [
                {"role": "ffmpeg", "path": "bin/ffmpeg.exe"},
                {"role": "ffprobe", "path": "bin/ffprobe.exe"}
            ],
            "resources": ["licenses/ffmpeg.txt"]
        });
        // These are signed inert canaries, deliberately not actual PE executables
        // or license acceptance evidence. No fixture ever launches a program.
        let contents = BTreeMap::from([
            (
                "bin/ffmpeg.exe".into(),
                b"FIXTURE_FFMPEG_MUST_NOT_EXECUTE".to_vec(),
            ),
            (
                "bin/ffprobe.exe".into(),
                b"FIXTURE_FFPROBE_MUST_NOT_EXECUTE".to_vec(),
            ),
            (
                "licenses/ffmpeg.txt".into(),
                b"FIXTURE_LICENSE_NOT_LEGAL_APPROVAL".to_vec(),
            ),
        ]);
        let mut fixture = Self {
            base,
            manifest,
            contents,
            method,
        };
        fixture.rebuild();
        fixture
    }
    fn rebuild(&mut self) {
        self.base.package.files = self
            .contents
            .iter()
            .map(|(path, bytes)| PackageFile {
                path: path.clone(),
                size: bytes.len() as u64,
                sha256: hash(bytes),
            })
            .collect();
        self.manifest["files"] = serde_json::to_value(&self.base.package.files).unwrap();
        self.base.manifest = serde_json::to_vec(&self.manifest).unwrap();
        self.repack();
    }
    fn repack(&mut self) {
        self.base.package.manifest = BlobDescriptor {
            size: self.base.manifest.len() as u64,
            sha256: hash(&self.base.manifest),
        };
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default().compression_method(self.method);
        zip.start_file("manifest.json", options).unwrap();
        zip.write_all(&self.base.manifest).unwrap();
        for (name, bytes) in &self.contents {
            zip.start_file(name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        self.base.archive = zip.finish().unwrap().into_inner();
        self.base.package.archive.size = self.base.archive.len() as u64;
        self.base.package.archive.sha256 = hash(&self.base.archive);
    }
    fn setup(&self) -> (DataStore, PackageTrustStore) {
        let mut store = DataStore::memory().unwrap();
        let trust = self.trust();
        self.bootstrap(&mut store, &trust);
        (store, trust)
    }
    fn trust(&self) -> PackageTrustStore {
        PackageTrustStore::new(BootstrapPins {
            keys: serde_json::from_value(self.base.golden["pins"].clone()).unwrap(),
            threshold: 2,
        })
        .unwrap()
    }
    fn bootstrap(&self, store: &mut DataStore, trust: &PackageTrustStore) {
        trust
            .bootstrap(
                store,
                &serde_json::to_vec(&self.base.golden["rootEnvelope"]).unwrap(),
                self.now(),
            )
            .unwrap();
    }
    fn now(&self) -> u64 {
        self.base.policy.clock.now
    }
    fn input<'a>(&'a self, envelope: &'a [u8]) -> ToolArchiveInput<'a> {
        ToolArchiveInput {
            envelope,
            manifest: &self.base.manifest,
            archive: &self.base.archive,
        }
    }
    fn admit(
        &self,
        store: &mut DataStore,
        trust: &PackageTrustStore,
    ) -> Result<InertToolArchive, ToolAdmissionError> {
        admit_fixture(
            trust,
            store,
            self.input(&signed(PACKAGE_PAYLOAD_TYPE, &self.base.package, &[3])),
            || self.now(),
        )
    }
    fn check(&self) -> Result<InertToolArchive, ToolAdmissionError> {
        let (mut store, trust) = self.setup();
        let result = self.admit(&mut store, &trust);
        assert_no_effects(&store);
        result
    }
}
fn assert_no_effects(store: &DataStore) {
    for table in ["grants", "jobs", "plugin_data", "imports"] {
        let count: u32 = store
            .connection
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(count, 0, "{table}");
    }
    let count: u32 = store
        .connection
        .query_row(
            "SELECT count(*) FROM core_metadata WHERE key LIKE 'package-install-%'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(count, 0);
}

#[test]
fn signed_stored_and_deflated_tools_return_only_original_inert_bytes_and_roles() {
    for method in [CompressionMethod::Stored, CompressionMethod::Deflated] {
        let fixture = ToolFixture::new(method);
        let result = fixture.check().unwrap();
        assert_eq!(
            result.programs(),
            &[ToolProgram::Ffmpeg, ToolProgram::Ffprobe]
        );
        assert_eq!(
            result.archive().descriptor().package_digest(),
            fixture.base.package.archive.sha256
        );
        assert_eq!(
            result.archive().files()["manifest.json"],
            fixture.base.manifest
        );
        assert_eq!(
            result.archive().files()["bin/ffmpeg.exe"],
            fixture.contents["bin/ffmpeg.exe"]
        );
        assert_eq!(result.archive().files().len(), 4);
    }
}

#[test]
fn production_review_list_denies_even_a_trusted_fully_signed_fixture() {
    let fixture = ToolFixture::new(CompressionMethod::Stored);
    let (mut store, trust) = fixture.setup();
    let result = admit_tool_archive(
        &trust,
        &mut store,
        fixture.input(&signed(PACKAGE_PAYLOAD_TYPE, &fixture.base.package, &[3])),
        || fixture.now(),
    );
    assert_eq!(result.err(), Some(ToolAdmissionError::PolicyDenied));
    assert_no_effects(&store);
}

#[test]
fn unknown_nested_fields_roles_missing_fields_and_duplicate_json_keys_refuse() {
    for (pointer, value) in [
        ("/formatVersion", json!(2)),
        ("/kind", json!("plugin")),
        ("/programs/0/role", json!("shell")),
        ("/programs", json!([])),
        (
            "/programs",
            json!([{"role":"ffmpeg","path":"bin/ffmpeg.exe","argv":["-i"]}]),
        ),
        (
            "/source",
            json!({"registry":"fixture-registry","artifact":"fixture-ffmpeg","url":"https://example.invalid"}),
        ),
    ] {
        let mut fixture = ToolFixture::new(CompressionMethod::Stored);
        *fixture.manifest.pointer_mut(pointer).unwrap() = value;
        fixture.rebuild();
        assert_eq!(
            fixture.check().err(),
            Some(ToolAdmissionError::ManifestInvalid),
            "{pointer}"
        );
    }
    for name in [
        "rawArgv",
        "installScript",
        "certified",
        "reviewed",
        "permissions",
        "environment",
        "workingDirectory",
    ] {
        let mut fixture = ToolFixture::new(CompressionMethod::Stored);
        fixture.manifest[name] = json!(true);
        fixture.rebuild();
        assert_eq!(
            fixture.check().err(),
            Some(ToolAdmissionError::ManifestInvalid),
            "{name}"
        );
    }
    for name in ["programs", "resources", "license", "buildFlavor"] {
        let mut fixture = ToolFixture::new(CompressionMethod::Stored);
        fixture.manifest.as_object_mut().unwrap().remove(name);
        fixture.rebuild();
        assert_eq!(
            fixture.check().err(),
            Some(ToolAdmissionError::ManifestInvalid),
            "{name}"
        );
    }
    let mut fixture = ToolFixture::new(CompressionMethod::Stored);
    fixture.base.manifest = format!(
        "{{\"formatVersion\":1,{}",
        serde_json::to_string(&fixture.manifest)
            .unwrap()
            .trim_start_matches('{')
    )
    .into_bytes();
    fixture.repack();
    assert!(fixture.check().is_err());
}

#[test]
fn signed_manifest_must_match_descriptor_source_license_and_flavor_exactly() {
    for pointer in [
        "/license",
        "/buildFlavor",
        "/source/registry",
        "/source/artifact",
    ] {
        let mut fixture = ToolFixture::new(CompressionMethod::Stored);
        *fixture.manifest.pointer_mut(pointer).unwrap() = json!("unreviewed");
        fixture.rebuild();
        assert_eq!(
            fixture.check().err(),
            Some(ToolAdmissionError::ManifestInvalid),
            "{pointer}"
        );
    }
}

#[test]
fn validly_signed_unreviewed_source_license_and_build_flavor_are_denied() {
    for variant in 0..4 {
        let mut fixture = ToolFixture::new(CompressionMethod::Stored);
        match variant {
            0 => {
                fixture.base.package.license = "MIT".into();
                fixture.manifest["license"] = json!("MIT");
            }
            1 => {
                fixture.base.package.build_flavor = "unreviewed-dynamic".into();
                fixture.manifest["buildFlavor"] = json!("unreviewed-dynamic");
            }
            2 => {
                fixture.base.package.source.registry = "unreviewed".into();
                fixture.manifest["source"]["registry"] = json!("unreviewed");
            }
            _ => {
                fixture.base.package.source.artifact = "unreviewed".into();
                fixture.manifest["source"]["artifact"] = json!("unreviewed");
            }
        }
        fixture.rebuild();
        assert_eq!(
            fixture.check().err(),
            Some(ToolAdmissionError::PolicyDenied)
        );
    }
}

#[test]
fn missing_duplicate_or_unreviewed_program_resource_roles_are_denied() {
    for (pointer, value) in [
        (
            "/programs",
            json!([{"role":"ffmpeg","path":"bin/ffmpeg.exe"}]),
        ),
        ("/programs/1/role", json!("ffmpeg")),
        ("/programs/0/path", json!("bin/ffprobe.exe")),
        ("/programs/0/path", json!("c:/windows/cmd.exe")),
        ("/programs/0/path", json!("../ffmpeg.exe")),
        ("/resources", json!([])),
        (
            "/resources",
            json!(["licenses/ffmpeg.txt", "licenses/ffmpeg.txt"]),
        ),
        ("/resources/0", json!("bin/ffmpeg.exe")),
    ] {
        let mut fixture = ToolFixture::new(CompressionMethod::Stored);
        *fixture.manifest.pointer_mut(pointer).unwrap() = value;
        fixture.rebuild();
        assert_eq!(
            fixture.check().err(),
            Some(ToolAdmissionError::PolicyDenied),
            "{pointer}"
        );
    }
}

#[test]
fn signed_extra_dll_helper_script_and_empty_license_cannot_be_adopted_as_resources() {
    for name in [
        "bin/avcodec.dll",
        "bin/helper.exe",
        "install.ps1",
        "resources/unknown.txt",
    ] {
        let mut fixture = ToolFixture::new(CompressionMethod::Stored);
        fixture
            .contents
            .insert(name.into(), b"MUST_NOT_EXECUTE".to_vec());
        fixture.manifest["resources"]
            .as_array_mut()
            .unwrap()
            .push(json!(name));
        fixture.rebuild();
        assert_eq!(
            fixture.check().err(),
            Some(ToolAdmissionError::PolicyDenied),
            "{name}"
        );
    }
    let mut fixture = ToolFixture::new(CompressionMethod::Stored);
    fixture
        .contents
        .insert("licenses/ffmpeg.txt".into(), vec![]);
    fixture.rebuild();
    assert_eq!(
        fixture.check().err(),
        Some(ToolAdmissionError::PolicyDenied)
    );
}

#[test]
fn plugin_kind_and_caller_claimed_foreign_target_do_not_enter_tool_admission() {
    let mut fixture = ToolFixture::new(CompressionMethod::Stored);
    fixture.base.package.kind = PackageKind::Plugin;
    fixture.base.package.id = "fixture-plugin".into();
    fixture.manifest["id"] = json!("fixture-plugin");
    fixture.manifest["targets"] = json!([fixture.base.package.target]);
    fixture.rebuild();
    assert_eq!(
        fixture.check().err(),
        Some(ToolAdmissionError::ManifestInvalid)
    );
    let mut fixture = ToolFixture::new(CompressionMethod::Stored);
    fixture.base.package.target.arch = PackageArch::Arm64;
    fixture.manifest["target"]["arch"] = json!("arm64");
    fixture.rebuild();
    assert_eq!(
        fixture.check().err(),
        Some(ToolAdmissionError::Package(PackageError::TargetMismatch))
    );
}

#[test]
fn changed_archive_manifest_and_wrong_signer_fail_before_tool_proof() {
    for variant in 0..3 {
        let mut fixture = ToolFixture::new(CompressionMethod::Stored);
        let (mut store, trust) = fixture.setup();
        let envelope = signed(
            PACKAGE_PAYLOAD_TYPE,
            &fixture.base.package,
            &[if variant == 2 { 4 } else { 3 }],
        );
        if variant == 0 {
            fixture.base.archive[0] ^= 1;
        }
        if variant == 1 {
            fixture.base.manifest.push(b' ');
        }
        let result = admit_fixture(&trust, &mut store, fixture.input(&envelope), || {
            fixture.now()
        });
        assert_eq!(
            result.err(),
            Some(ToolAdmissionError::Package(if variant == 2 {
                PackageError::SignatureInvalid
            } else {
                PackageError::IntegrityMismatch
            }))
        );
        assert!(trust
            .snapshot(&mut store)
            .unwrap()
            .known_versions
            .is_empty());
        assert_no_effects(&store);
    }
}

#[test]
fn valid_signature_over_hostile_zip_still_fails_archive_admission() {
    let mut fixture = ToolFixture::new(CompressionMethod::Stored);
    // ZIP32 end-of-central-directory comment field: signing the modified ZIP
    // cannot make an archive outside the existing strict install profile valid.
    let end = fixture.base.archive.len() - 22;
    fixture.base.archive[end + 20] = 1;
    fixture.base.package.archive.sha256 = hash(&fixture.base.archive);
    assert_eq!(
        fixture.check().err(),
        Some(ToolAdmissionError::Package(PackageError::ArchiveInvalid))
    );
}

#[test]
fn tool_budgets_refuse_before_decode_and_preserve_conservative_observations() {
    let mut fixture = ToolFixture::new(CompressionMethod::Stored);
    fixture.base.manifest.resize(65_537, b' ');
    let (mut store, trust) = fixture.setup();
    let before = trust.snapshot(&mut store).unwrap();
    assert_eq!(
        fixture.admit(&mut store, &trust).err(),
        Some(ToolAdmissionError::ManifestBudgetExceeded)
    );
    assert_eq!(
        trust.snapshot(&mut store).unwrap().observed_time,
        before.observed_time
    );
    assert!(trust
        .snapshot(&mut store)
        .unwrap()
        .known_versions
        .is_empty());
    let mut fixture = ToolFixture::new(CompressionMethod::Stored);
    for index in 0..30 {
        fixture
            .contents
            .insert(format!("resources/file-{index}.txt"), vec![1]);
    }
    fixture.rebuild();
    let (mut store, trust) = fixture.setup();
    assert_eq!(
        fixture.admit(&mut store, &trust).err(),
        Some(ToolAdmissionError::ManifestBudgetExceeded)
    );
    assert_eq!(trust.snapshot(&mut store).unwrap().known_versions.len(), 1);
    assert_no_effects(&store);
}

#[test]
fn expiry_or_clock_rollback_at_final_check_cannot_return_an_earlier_receipt() {
    for final_time in [0, 1] {
        let fixture = ToolFixture::new(CompressionMethod::Stored);
        let (mut store, trust) = fixture.setup();
        let envelope = signed(PACKAGE_PAYLOAD_TYPE, &fixture.base.package, &[3]);
        let mut samples = 0;
        let result = admit_fixture(&trust, &mut store, fixture.input(&envelope), || {
            samples += 1;
            if samples == 1 {
                fixture.now()
            } else if final_time == 0 {
                fixture.now() - 1
            } else {
                fixture.base.package.expires_at
            }
        });
        assert_eq!(samples, 2);
        assert_eq!(
            result.err(),
            Some(ToolAdmissionError::Package(if final_time == 0 {
                PackageError::ClockRollback
            } else {
                PackageError::TrustExpired
            }))
        );
        assert_eq!(trust.snapshot(&mut store).unwrap().known_versions.len(), 1);
        assert_no_effects(&store);
    }
}

#[test]
fn root_rotation_revocation_requires_current_verification_and_refuses_the_tool() {
    let fixture = ToolFixture::new(CompressionMethod::Stored);
    let (mut store, trust) = fixture.setup();
    let admitted = fixture.admit(&mut store, &trust).unwrap();
    let envelope = serde_json::to_vec(&fixture.base.golden["rootEnvelope"]).unwrap();
    let mut rotated: RootPayload = serde_json::from_slice(
        &decode_envelope(&envelope, ROOT_PAYLOAD_TYPE)
            .unwrap()
            .payload,
    )
    .unwrap();
    rotated.version += 1;
    rotated.previous_root_sha256 = Some(trust.snapshot(&mut store).unwrap().root_digest);
    rotated
        .revoked_package_digests
        .push(admitted.archive().descriptor().package_digest().into());
    trust
        .rotate(
            &mut store,
            &signed(ROOT_PAYLOAD_TYPE, &rotated, &[1, 2]),
            fixture.now(),
        )
        .unwrap();
    assert_eq!(
        fixture.admit(&mut store, &trust).err(),
        Some(ToolAdmissionError::Package(PackageError::PackageRevoked))
    );
    assert_no_effects(&store);
}

#[test]
fn trust_write_failure_and_recovery_marker_never_return_tool_proof() {
    let fixture = ToolFixture::new(CompressionMethod::Stored);
    let (mut store, trust) = fixture.setup();
    store.connection.execute_batch("CREATE TRIGGER refuse_tool BEFORE UPDATE ON core_metadata BEGIN SELECT RAISE(ABORT,'fixture'); END;").unwrap();
    assert_eq!(
        fixture.admit(&mut store, &trust).err(),
        Some(ToolAdmissionError::Package(
            PackageError::TrustStorageFailed
        ))
    );
    assert!(trust
        .snapshot(&mut store)
        .unwrap()
        .known_versions
        .is_empty());
    store
        .connection
        .execute_batch("DROP TRIGGER refuse_tool;")
        .unwrap();
    store.connection.execute("INSERT INTO core_metadata(key,value) VALUES('package-trust-recovery-required','fixture')", []).unwrap();
    assert_eq!(
        fixture.admit(&mut store, &trust).err(),
        Some(ToolAdmissionError::Package(
            PackageError::TrustRecoveryRequired
        ))
    );
    assert_no_effects(&store);
}

#[test]
fn observed_signed_identity_survives_disk_reopen_without_installing_or_executing() {
    let fixture = ToolFixture::new(CompressionMethod::Stored);
    let root = std::env::temp_dir().join(format!(
        "flowtools-validation-tool-admission-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&root).unwrap();
    let db = root.join("runtime.sqlite");
    let trust = fixture.trust();
    {
        let mut store = DataStore::open(&db).unwrap();
        fixture.bootstrap(&mut store, &trust);
        fixture.admit(&mut store, &trust).unwrap();
        assert_no_effects(&store);
    }
    {
        let mut store = DataStore::open(&db).unwrap();
        let snapshot = trust.snapshot(&mut store).unwrap();
        assert_eq!(snapshot.known_versions.len(), 1);
        assert_eq!(snapshot.known_versions[0].kind, PackageKind::Tool);
        assert_eq!(
            snapshot.known_versions[0].package_digest,
            fixture.base.package.archive.sha256
        );
        fixture.admit(&mut store, &trust).unwrap();
        assert!(!root.join("package-store-v1").exists());
        assert_no_effects(&store);
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn tool_versions_keep_exact_zip_identity_and_release_floors_after_policy_refusal() {
    let fixture = ToolFixture::new(CompressionMethod::Stored);
    let (mut store, trust) = fixture.setup();
    fixture.admit(&mut store, &trust).unwrap();
    let mut replacement = ToolFixture::new(CompressionMethod::Stored);
    replacement
        .contents
        .insert("bin/ffmpeg.exe".into(), b"DIFFERENT_SIGNED_BYTES".to_vec());
    replacement.rebuild();
    assert_eq!(
        replacement.admit(&mut store, &trust).err(),
        Some(ToolAdmissionError::Package(PackageError::VersionConflict))
    );
    let mut newer = ToolFixture::new(CompressionMethod::Stored);
    newer.base.package.version = "9.0.0".into();
    newer.base.package.release_sequence += 1;
    newer.manifest["version"] = json!("9.0.0");
    newer.base.package.build_flavor = "unreviewed".into();
    newer.manifest["buildFlavor"] = json!("unreviewed");
    newer.rebuild();
    assert_eq!(
        newer.admit(&mut store, &trust).err(),
        Some(ToolAdmissionError::PolicyDenied)
    );
    // A valid signed observation remains durable even when later layout/review
    // refuses. It is never rolled back to make the older package usable again.
    assert_eq!(trust.snapshot(&mut store).unwrap().known_versions.len(), 2);
    assert_eq!(
        fixture.admit(&mut store, &trust).err(),
        Some(ToolAdmissionError::Package(PackageError::ReleaseRollback))
    );
    assert_no_effects(&store);
}
