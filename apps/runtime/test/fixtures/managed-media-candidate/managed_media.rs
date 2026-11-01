//! Explicit signed-fixture/installed-image bridge. Not a product media adapter.
#[path = "native_inspection.rs"]
mod native_inspection;
use crate::packages::tools::native_fixture as harness;
use crate::{
    broker::Scope,
    catalog::{digest, BuiltinCatalog},
    data::DataStore,
    packages::{
        self,
        tools::{
            self,
            install::{self, images},
            media::{self, MediaActionV1},
            ToolArchiveInput, ToolProgram,
        },
        trust::PackageTrustStore,
        BlobDescriptor, BootstrapPins, PackageFile, PackageKind, PackagePayload, RootPayload,
        PACKAGE_PAYLOAD_TYPE, ROOT_PAYLOAD_TYPE,
    },
    protocol::*,
    runtime::{now, RuntimeCore},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use ed25519_dalek::{Signer, SigningKey};
use harness::{Fixture, InstalledMediaProgram};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashMap},
    fs::{self, OpenOptions},
    io::{Cursor, Read, Write},
    os::windows::{fs::OpenOptionsExt, io::AsRawHandle},
    path::{Path, PathBuf},
    sync::{atomic::AtomicU64, Arc},
    time::Duration,
};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

const FFMPEG: &[u8] = include_bytes!("../media-candidate/.generated/bin/ffmpeg.exe");
const FFPROBE: &[u8] = include_bytes!("../media-candidate/.generated/bin/ffprobe.exe");
const NOTICES: &[u8] = include_bytes!(".generated/notices.txt");
const ACTIONS: &[&str] = &["inspect-v1", "extract-audio-wav-v1", "transcode-webm-v1"];
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn signed(payload_type: &str, value: &impl Serialize, seeds: &[u8]) -> Vec<u8> {
    let bytes = serde_json::to_vec(value).unwrap();
    let mut pae = format!(
        "DSSEv1 {} {} {} ",
        payload_type.len(),
        payload_type,
        bytes.len()
    )
    .into_bytes();
    pae.extend_from_slice(&bytes);
    let signatures: Vec<_> = seeds.iter().map(|seed| {
        let key = SigningKey::from_bytes(&[*seed;32]);
        json!({"keyid":hash(&key.verifying_key().to_bytes()), "sig":STANDARD.encode(key.sign(&pae).to_bytes())})
    }).collect();
    serde_json::to_vec(&json!({"payloadType":payload_type,"payload":STANDARD.encode(bytes),"signatures":signatures})).unwrap()
}
fn candidate(role: &str, bytes: &[u8]) -> Result<(), &'static str> {
    let lock: Value =
        serde_json::from_str(include_str!("../media-candidate/candidate-lock.json")).unwrap();
    if bytes.len() < 1024
        || bytes.len() > 33_554_432
        || bytes.get(..2) != Some(b"MZ")
        || hash(bytes) != lock["programs"][role]
    {
        return Err("Exact candidate image required");
    }
    Ok(())
}
struct SignedFixture {
    golden: Value,
    root: RootPayload,
    package: PackagePayload,
    manifest: Vec<u8>,
    archive: Vec<u8>,
}
impl SignedFixture {
    fn new() -> Self {
        candidate("ffmpeg", FFMPEG).unwrap();
        candidate("ffprobe", FFPROBE).unwrap();
        let notice_lock: Value = serde_json::from_str(include_str!("notice-lock.json")).unwrap();
        assert_eq!(hash(NOTICES), notice_lock["combinedSha256"]);
        let golden: Value = serde_json::from_str(include_str!(
            "../../../../../packages/sdk/test/fixtures/signed-package-v1.json"
        ))
        .unwrap();
        let mut root: RootPayload = serde_json::from_slice(
            &STANDARD
                .decode(golden["rootEnvelope"]["payload"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        let mut package: PackagePayload =
            serde_json::from_value(golden["descriptor"].clone()).unwrap();
        package.kind = PackageKind::Tool;
        package.id = "ffmpeg".into();
        package.source.registry = "fixture-registry".into();
        package.source.artifact = "fixture-ffmpeg".into();
        package.license = "LGPL-2.1-or-later".into();
        package.build_flavor = "fixture-static".into();
        root.publishers[0].packages.push(packages::PublisherScope {
            kind: PackageKind::Tool,
            id: "ffmpeg".into(),
            targets: vec![package.target.clone()],
        });
        let entries = BTreeMap::from([
            ("bin/ffmpeg.exe", FFMPEG),
            ("bin/ffprobe.exe", FFPROBE),
            ("licenses/ffmpeg.txt", NOTICES),
        ]);
        package.files = entries
            .iter()
            .map(|(path, bytes)| PackageFile {
                path: (*path).into(),
                size: bytes.len() as u64,
                sha256: hash(bytes),
            })
            .collect();
        let manifest = serde_json::to_vec(&json!({
            "formatVersion":1, "kind":"tool", "publisher":package.publisher,"id":package.id,
            "version":package.version,"target":package.target,"buildFlavor":package.build_flavor,
            "source":package.source,"license":package.license,"files":package.files,
            "programs":[{"role":"ffmpeg","path":"bin/ffmpeg.exe"},{"role":"ffprobe","path":"bin/ffprobe.exe"}],
            "resources":["licenses/ffmpeg.txt"]
        })).unwrap();
        package.manifest = BlobDescriptor {
            size: manifest.len() as u64,
            sha256: hash(&manifest),
        };
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        for (path, bytes) in std::iter::once(("manifest.json", manifest.as_slice())).chain(entries)
        {
            zip.start_file(
                path,
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(bytes).unwrap();
        }
        let archive = zip.finish().unwrap().into_inner();
        package.archive.size = archive.len() as u64;
        package.archive.sha256 = hash(&archive);
        Self {
            golden,
            root,
            package,
            manifest,
            archive,
        }
    }
    fn clock(&self) -> u64 {
        self.golden["now"].as_u64().unwrap()
    }
    fn trust(&self) -> PackageTrustStore {
        PackageTrustStore::new(BootstrapPins {
            keys: serde_json::from_value(self.golden["pins"].clone()).unwrap(),
            threshold: 2,
        })
        .unwrap()
    }
    fn input<'a>(&'a self, envelope: &'a [u8]) -> ToolArchiveInput<'a> {
        ToolArchiveInput {
            envelope,
            manifest: &self.manifest,
            archive: &self.archive,
        }
    }
}
struct World {
    fixture: SignedFixture,
    path: PathBuf,
    core: RuntimeCore,
    catalog: BuiltinCatalog,
    profile: String,
    image: PathBuf,
    app_sids: Vec<String>,
    pids: Vec<u32>,
    sessions: HashMap<String, SessionProof>,
}
impl World {
    fn new() -> Self {
        let fixture = SignedFixture::new();
        let path =
            std::env::temp_dir().join(format!("flowtools-managed-media-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
        let trust = fixture.trust();
        trust
            .bootstrap(
                &mut store,
                &signed(ROOT_PAYLOAD_TYPE, &fixture.root, &[1, 2]),
                fixture.clock(),
            )
            .unwrap();
        let envelope = signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]);
        let installed = install::install_fixture_with_hook(
            &trust,
            &mut store,
            fixture.input(&envelope),
            || fixture.clock(),
            |_, _| Ok(()),
        )
        .unwrap();
        let held = images::materialize_fixture_with_hook(
            &trust,
            &mut store,
            &installed,
            || fixture.clock(),
            |_, _| Ok(()),
        )
        .unwrap();
        let image = path
            .join("tool-image-store-v1/artifacts")
            .join(held.pin().archive_digest);
        drop(held);
        let inventory: String = store
            .connection
            .query_row(
                "SELECT value FROM core_metadata WHERE key='tool-image-inventory-v1'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        let profile = serde_json::from_str::<Value>(&inventory).unwrap()["profileDigest"]
            .as_str()
            .unwrap()
            .to_owned();
        let base: Value = serde_json::from_str(include_str!(
            "../../../../../packages/sdk/test/fixtures/manifest-admission-v1.json"
        ))
        .unwrap();
        let manifests=["consumer","other-consumer"].into_iter().map(|id|{
            let mut value=base["base"].clone();
            value["publisher"]=json!("fixture-publisher"); value["id"]=json!(id); value["version"]=json!("1.0.0");
            value["commands"][0]["effects"]=json!(["tool-execute"]);
            value["commands"][0]["permissions"]=json!([{"capability":"tool","operations":["execute"],"scopes":ACTIONS.iter().map(|a|format!("lock:ffmpeg:{a}")).collect::<Vec<_>>()}]);
            value["dependencies"]["tools"]=json!([{"publisher":"fixture-publisher","id":"ffmpeg","version":"*","target":fixture.package.target,"buildFlavor":"fixture-static","digest":fixture.package.archive.sha256}]);
            value
        }).collect();
        let catalog = BuiltinCatalog::from_host_manifests(manifests).unwrap();
        let core = RuntimeCore::managed_fixture_tool_catalog(
            HashMap::from([
                ("cli".into(), "local-cli".into()),
                ("admin".into(), "local-manager".into()),
            ]),
            store,
            false,
            catalog.clone(),
            trust,
            Arc::new(AtomicU64::new(fixture.clock())),
        )
        .unwrap();
        Self {
            fixture,
            path,
            core,
            catalog,
            profile,
            image,
            app_sids: Vec::new(),
            pids: Vec::new(),
            sessions: HashMap::new(),
        }
    }
    fn open(&mut self, token: &str) -> SessionProof {
        if let Some(session) = self.sessions.get(token) {
            return session.clone();
        }
        let response = self.core.handle(
            token,
            request(
                Call::Open(OpenSession {
                    token: token.into(),
                    client_version: CLIENT_VERSION.into(),
                    expected_instance_id: None,
                }),
                None,
            ),
        );
        let Outcome::Session(session) = response.outcome else {
            panic!("{response:?}")
        };
        self.sessions.insert(token.into(), session.clone());
        session
    }
    fn grant(&mut self, id: &str) {
        self.grant_limit(id, 8);
    }
    fn grant_limit(&mut self, id: &str, max_calls: u32) {
        let admin = self.open("admin");
        let (manifest, _) = self.catalog.permission_operation(id, "convert").unwrap();
        let response = self.core.handle(
            "admin",
            request(
                Call::Grant(PermissionGrant {
                    plugin_id: id.into(),
                    command_id: "convert".into(),
                    target: GrantTarget::Cli,
                    package_digest: digest(manifest),
                    effects: vec!["tool-execute".into()],
                    scopes: ACTIONS
                        .iter()
                        .map(|a| Scope::ToolLock {
                            lock: "ffmpeg".into(),
                            action: (*a).into(),
                        })
                        .collect(),
                    expires_at: now() + 60_000.0,
                    max_calls,
                    cold_start: false,
                    background: false,
                }),
                Some(admin),
            ),
        );
        assert!(
            matches!(response.outcome, Outcome::Permissions(_)),
            "{response:?}"
        );
    }
    fn start(&mut self, id: &str) -> String {
        self.grant(id);
        self.start_granted(id)
    }
    fn start_granted(&mut self, id: &str) -> String {
        let cli = self.open("cli");
        let response = self.core.handle(
            "cli",
            request(
                Call::Submit(SubmitJob {
                    plugin_id: id.into(),
                    command_id: "convert".into(),
                    input: json!({"text":"MANAGED_MEDIA_PRIVATE_INPUT"}),
                    idempotency_key: uuid::Uuid::new_v4().to_string(),
                    background: false,
                    deadline: now() + 20_000.0,
                }),
                Some(cli),
            ),
        );
        let Outcome::Receipt(receipt) = response.outcome else {
            panic!("{response:?}")
        };
        assert!(self.core.take_run(&receipt.run_id).is_some());
        receipt.run_id
    }
    fn row(&mut self, id: &str) -> Value {
        self.core.with_tool_fixture_store(|store| {
            let raw: String = store
                .connection
                .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [id], |r| {
                    r.get(0)
                })
                .unwrap();
            assert!(!raw.contains("MANAGED_MEDIA_PRIVATE_INPUT"));
            serde_json::from_str(&raw).unwrap()
        })
    }
    fn run(
        &mut self,
        id: &str,
        fixture: &Fixture,
        action: MediaActionV1,
    ) -> (u32, Vec<u8>, Vec<u8>) {
        let profile = &self.profile;
        let archive = &self.fixture.package.archive.sha256;
        let image = &self.image;
        let (output, pid) = self
            .core
            .invoke_media_tool(id, action, |tool| {
                assert_eq!(tool.tool().digest, *archive);
                assert_eq!(tool.tool().consumer, tool.consumer().identity());
                let role = match tool.program() {
                    ToolProgram::Ffmpeg => InstalledMediaProgram::Ffmpeg,
                    ToolProgram::Ffprobe => InstalledMediaProgram::Ffprobe,
                };
                let child = fixture
                    .launch_installed_media(
                        profile,
                        archive,
                        image,
                        role,
                        media::recipe(action).arguments(),
                    )
                    .unwrap();
                assert_ne!(child.pub_pid, 0);
                assert_eq!(child.child_policy, 1);
                assert_eq!(child.active_limit, 1);
                assert!(!child.private_inherited);
                let expected = image.join(match tool.program() {
                    ToolProgram::Ffmpeg => "bin/ffmpeg.exe",
                    ToolProgram::Ffprobe => "bin/ffprobe.exe",
                });
                assert_eq!(
                    fs::canonicalize(
                        child
                            .admitted_image
                            .as_ref()
                            .expect("Image read back before resume")
                    )
                    .unwrap(),
                    fs::canonicalize(expected).unwrap()
                );
                let output = child.output(Duration::from_secs(5), 65_536).unwrap();
                child.wait_empty(Duration::from_millis(500)).unwrap();
                assert_eq!(child.active().unwrap(), 0);
                assert_eq!(
                    fs::read(fixture.host.join("private.txt")).unwrap(),
                    b"HOST_PRIVATE_CANARY"
                );
                Ok((output, child.pub_pid))
            })
            .unwrap();
        self.app_sids.push(fixture.container.text.clone());
        self.pids.push(pid);
        output
    }
    fn probe(&mut self, id: &str, bytes: &[u8]) -> Vec<u8> {
        let fixture = Fixture::new().unwrap();
        fs::write(fixture.work.join("input"), bytes).unwrap();
        let (code, output, _) = self.run(id, &fixture, MediaActionV1::Inspect);
        assert_eq!(code, 0);
        output
    }
}
fn request(call: Call, session: Option<SessionProof>) -> Request {
    Request {
        version: 1,
        request_id: uuid::Uuid::new_v4().to_string(),
        session,
        call,
    }
}
fn wave() -> Vec<u8> {
    let mut data = Vec::new();
    let pcm: Vec<_> = (0..16_000u32)
        .flat_map(|i| ((i.wrapping_mul(127) % 30_000) as i16 - 15_000).to_le_bytes())
        .collect();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    data.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    data.extend_from_slice(&16_000u32.to_le_bytes());
    data.extend_from_slice(&32_000u32.to_le_bytes());
    data.extend_from_slice(b"\x02\0\x10\0data");
    data.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    data.extend_from_slice(&pcm);
    data
}
// Open the actual fresh output after real exit/Job drain; hold it without sharing
// writes/deletes, reject links/reparse, and compare native identity after reading.
fn output(path: &Path) -> (Vec<u8>, u64) {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_REPARSE_POINT,
        FILE_FLAG_OPEN_REPARSE_POINT, FILE_SHARE_READ,
    };
    let mut file = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ)
        .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
        .open(path)
        .unwrap();
    let mut before = BY_HANDLE_FILE_INFORMATION::default();
    assert_ne!(
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut before) },
        0
    );
    assert_eq!(before.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT, 0);
    assert_eq!(before.nNumberOfLinks, 1);
    let size = (u64::from(before.nFileSizeHigh) << 32) | u64::from(before.nFileSizeLow);
    assert!(size > 0 && size <= 67_108_864);
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(67_108_865)
        .read_to_end(&mut bytes)
        .unwrap();
    assert_eq!(bytes.len() as u64, size);
    let mut after = BY_HANDLE_FILE_INFORMATION::default();
    assert_ne!(
        unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut after) },
        0
    );
    assert_eq!(
        (
            before.dwVolumeSerialNumber,
            before.nFileIndexHigh,
            before.nFileIndexLow,
            before.nFileSizeHigh,
            before.nFileSizeLow
        ),
        (
            after.dwVolumeSerialNumber,
            after.nFileIndexHigh,
            after.nFileIndexLow,
            after.nFileSizeHigh,
            after.nFileSizeLow
        )
    );
    (bytes, size)
}

#[test]
fn exact_real_images_still_fail_empty_production_review_before_installation() {
    assert!(candidate("ffmpeg", b"MZ").is_err());
    let mut altered = FFMPEG.to_vec();
    altered[1024] ^= 1;
    assert!(candidate("ffmpeg", &altered).is_err());
    let fixture = SignedFixture::new();
    let path = std::env::temp_dir().join(format!(
        "flowtools-managed-media-refused-{}",
        uuid::Uuid::new_v4()
    ));
    fs::create_dir(&path).unwrap();
    let mut store = DataStore::open(&path.join("runtime.sqlite")).unwrap();
    let trust = fixture.trust();
    trust
        .bootstrap(
            &mut store,
            &signed(ROOT_PAYLOAD_TYPE, &fixture.root, &[1, 2]),
            fixture.clock(),
        )
        .unwrap();
    let envelope = signed(PACKAGE_PAYLOAD_TYPE, &fixture.package, &[3]);
    assert!(matches!(
        install::install_inert_tool(&trust, &mut store, fixture.input(&envelope), || fixture
            .clock()),
        Err(install::ToolInstallError::Admission(
            tools::ToolAdmissionError::PolicyDenied
        ))
    ));
    assert!(!path.join("tool-package-store-v1").exists());
    assert_eq!(
        store
            .connection
            .query_row("SELECT count(*) FROM jobs", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
}
#[test]
#[allow(clippy::permissions_set_readonly_false)]
fn two_actual_consumers_share_one_signed_installed_image_and_inspect_in_distinct_lpacs() {
    let mut world = World::new();
    let first = world.start("consumer");
    let second = world.start("other-consumer");
    let a = world.row(&first);
    let b = world.row(&second);
    assert_eq!(a["toolArtifacts"]["images"], b["toolArtifacts"]["images"]);
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/artifacts"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        hash(&fs::read(world.image.join("bin/ffprobe.exe")).unwrap()),
        hash(FFPROBE)
    );
    for id in [&first, &second] {
        let metadata = media::parse_inspection(&world.probe(id, &wave())).unwrap();
        assert_eq!(metadata.formats, ["wav"]);
        assert_eq!(metadata.duration_ms, Some(1000));
    }
    assert_eq!(world.app_sids.len(), 2);
    assert_ne!(world.app_sids[0], world.app_sids[1]);
    assert_eq!(world.pids.len(), 2);
    world.core.finish(&first, Ok(json!("fixture")));
    assert_eq!(world.row(&first)["toolArtifacts"]["retained"], false);
    assert_eq!(world.row(&second)["toolArtifacts"]["retained"], true);
    let binary = world.image.join("bin/ffprobe.exe");
    let mut permissions = fs::metadata(&binary).unwrap().permissions();
    permissions.set_readonly(false);
    fs::set_permissions(&binary, permissions).unwrap();
    assert!(OpenOptions::new().write(true).open(&binary).is_err());
    world.core.finish(&second, Ok(json!("fixture")));
    let _writer = OpenOptions::new().write(true).open(&binary).unwrap();
}
#[test]
fn signed_installed_ffmpeg_extracts_fresh_wav_then_independent_ffprobe_validates_it() {
    let mut world = World::new();
    let id = world.start("consumer");
    let fixture = Fixture::new().unwrap();
    fs::write(fixture.work.join("input"), wave()).unwrap();
    let target = fixture.work.join("output.wav");
    assert!(!target.exists());
    let (code, progress, _) = world.run(&id, &fixture, MediaActionV1::ExtractAudioWav);
    let (bytes, size) = output(&target);
    let probe = world.probe(&id, &bytes);
    let metadata = media::completion::project_completed_output(
        MediaActionV1::ExtractAudioWav,
        code,
        &progress,
        &probe,
        size,
    )
    .unwrap();
    assert_eq!(metadata.formats, ["wav"]);
    assert_eq!(metadata.duration_ms, Some(1000));
    world.core.finish(&id, Ok(json!("fixture")));
}
#[path = "../media-candidate/video_input.rs"]
mod video_input;
#[test]
fn signed_installed_webm_and_exit_zero_overwrite_refusal_keep_exact_output_bytes() {
    let mut world = World::new();
    let id = world.start("consumer");
    let fixture = Fixture::new().unwrap();
    fs::write(fixture.work.join("input"), video_input::video(3, 16)).unwrap();
    let target = fixture.work.join("output.webm");
    assert!(!target.exists());
    let (code, progress, _) = world.run(&id, &fixture, MediaActionV1::TranscodeWebm);
    let (bytes, size) = output(&target);
    let probe = world.probe(&id, &bytes);
    let metadata = media::completion::project_completed_output(
        MediaActionV1::TranscodeWebm,
        code,
        &progress,
        &probe,
        size,
    )
    .unwrap();
    assert_eq!(metadata.formats, ["matroska", "webm"]);
    let (code, progress, _) = world.run(&id, &fixture, MediaActionV1::TranscodeWebm);
    assert_eq!(code, 0);
    assert!(media::completion::project_completed_output(
        MediaActionV1::TranscodeWebm,
        code,
        &progress,
        &probe,
        size
    )
    .is_err());
    assert_eq!(fs::read(target).unwrap(), bytes);
    world.core.finish(&id, Ok(json!("fixture")));
}
#[test]
fn revocation_after_real_image_io_never_reaches_the_native_launch_callback() {
    let mut world = World::new();
    let id = world.start("consumer");
    let admin = world.open("admin");
    let entered = std::cell::Cell::new(false);
    let result = world.core.invoke_media_fixture_with_hook(
        &id,
        MediaActionV1::Inspect,
        |_| {
            entered.set(true);
            panic!("Revoked tool callback")
        },
        |core| {
            let response = core.handle(
                "admin",
                request(
                    Call::Revoke(PermissionKey {
                        plugin_id: "consumer".into(),
                        command_id: "convert".into(),
                        target: GrantTarget::Cli,
                    }),
                    Some(admin),
                ),
            );
            assert!(matches!(response.outcome, Outcome::Permissions(_)));
            Ok(())
        },
    );
    assert_eq!(result, Err::<(), _>(ErrorCode::GrantRevoked));
    assert!(!entered.get());
    assert_eq!(world.row(&id)["toolArtifacts"]["retained"], true);
    world.core.finish(&id, Err(ErrorCode::GrantRevoked));
}
#[test]
fn current_signed_revocation_refuses_real_installed_candidates_without_erasing_the_roots() {
    let mut world = World::new();
    let id = world.start("consumer");
    let mut root = world.fixture.root.clone();
    root.version += 1;
    let trusted = world.fixture.trust();
    root.previous_root_sha256 = Some(
        world
            .core
            .with_tool_fixture_store(|store| trusted.snapshot(store).unwrap().root_digest),
    );
    root.revoked_package_digests
        .push(world.fixture.package.archive.sha256.clone());
    let clock = world.fixture.clock();
    world.core.with_tool_fixture_store(|store| {
        trusted
            .rotate(store, &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]), clock)
            .unwrap()
    });
    let result = world
        .core
        .invoke_media_tool::<()>(&id, MediaActionV1::Inspect, |_| {
            panic!("Revoked signed candidate")
        });
    assert_eq!(result, Err(ErrorCode::DependencyArtifactMismatch));
    assert_eq!(world.row(&id)["toolArtifacts"]["retained"], true);
    assert!(world.image.is_dir());
}
