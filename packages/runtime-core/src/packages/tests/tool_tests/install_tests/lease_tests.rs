//! Real NTFS artifacts retained through the ordinary fixed T1 Core job path.
//! Inert canary entries are never imported and no tool process is authorized.
use super::*;
use crate::{
    broker::CapabilityOperation,
    catalog::{digest, BuiltinCatalog},
    protocol::*,
    runtime::{now, RuntimeCore},
};
use std::{
    collections::HashMap,
    sync::{atomic::AtomicU64, Arc},
};
mod crash_tests;
mod drain_tests;
mod guard_tests;
mod policy_tests;
mod recovery_tests;

fn tokens() -> HashMap<String, String> {
    HashMap::from([
        ("cli".into(), "local-cli".into()),
        ("admin".into(), "local-manager".into()),
    ])
}
fn catalog(world: &World, ids: &[&str]) -> BuiltinCatalog {
    let manifests = ids
        .iter()
        .map(|id| {
            let mut value = super::super::super::dependency_tests::manifest(id, vec![]);
            value["dependencies"]["tools"] = json!([{
                "publisher": world.fixture.base.package.publisher, "id":"ffmpeg", "version":"*",
                "target": world.fixture.base.package.target, "buildFlavor":"fixture-static",
                "digest": world.fixture.base.package.archive.sha256,
            }]);
            let (_, manifest, _) =
                super::super::super::dependency_tests::bytes(&world.fixture.base, value, 1);
            serde_json::from_slice(&manifest).unwrap()
        })
        .collect();
    BuiltinCatalog::from_host_manifests(manifests).unwrap()
}
fn prepare_image(world: &mut World) -> PathBuf {
    let installed = world.install().unwrap();
    let now = world.fixture.now();
    let held = crate::packages::tools::install::images::materialize_fixture_with_hook(
        &world.trust,
        world.store.as_mut().unwrap(),
        &installed,
        || now,
        |_, _| Ok(()),
    )
    .unwrap();
    let path = world
        .path
        .join("tool-image-store-v1/artifacts")
        .join(held.pin().archive_digest);
    drop(held);
    path
}
fn core(world: &mut World, catalog: BuiltinCatalog, management: bool) -> RuntimeCore {
    core_with_clock(world, catalog, management).0
}
fn core_with_clock(
    world: &mut World,
    catalog: BuiltinCatalog,
    management: bool,
) -> (RuntimeCore, Arc<AtomicU64>) {
    let replacement = world.fixture.trust();
    let trust = std::mem::replace(&mut world.trust, replacement);
    let clock = Arc::new(AtomicU64::new(world.fixture.now()));
    let core = RuntimeCore::managed_fixture_tool_catalog(
        tokens(),
        world.store.take().unwrap(),
        management,
        catalog,
        trust,
        clock.clone(),
    )
    .unwrap();
    (core, clock)
}
fn request(call: Call, session: Option<SessionProof>) -> Request {
    Request {
        version: 1,
        request_id: uuid::Uuid::new_v4().to_string(),
        session,
        call,
    }
}
fn open(core: &mut RuntimeCore, token: &str) -> SessionProof {
    open_on(core, token, token)
}
fn open_on(core: &mut RuntimeCore, connection: &str, token: &str) -> SessionProof {
    let outcome = core
        .handle(
            connection,
            request(
                Call::Open(OpenSession {
                    token: token.into(),
                    client_version: CLIENT_VERSION.into(),
                    expected_instance_id: None,
                }),
                None,
            ),
        )
        .outcome;
    let Outcome::Session(session) = outcome else {
        panic!("{outcome:?}");
    };
    session
}
fn grant(core: &mut RuntimeCore, catalog: &BuiltinCatalog, id: &str) {
    grant_command(core, catalog, id, "convert");
}
fn grant_command(core: &mut RuntimeCore, catalog: &BuiltinCatalog, id: &str, command: &str) {
    let connection = format!("admin-{id}");
    let admin = open_on(core, &connection, "admin");
    let (manifest, _) = catalog.permission_operation(id, command).unwrap();
    let outcome = core
        .handle(
            &connection,
            request(
                Call::Grant(PermissionGrant {
                    plugin_id: id.into(),
                    command_id: command.into(),
                    target: GrantTarget::Cli,
                    package_digest: digest(manifest),
                    effects: vec![],
                    scopes: vec![],
                    expires_at: now() + 60_000.0,
                    max_calls: 16,
                    cold_start: false,
                    background: true,
                }),
                Some(admin),
            ),
        )
        .outcome;
    assert!(matches!(outcome, Outcome::Permissions(_)), "{outcome:?}");
    core.disconnect(&connection);
}
fn operation(id: &str, key: &str) -> SubmitJob {
    SubmitJob {
        plugin_id: id.into(),
        command_id: "convert".into(),
        input: json!({"text":"PRIVATE_TOOL_JOB_INPUT_CANARY"}),
        idempotency_key: key.into(),
        deadline: now() + 20_000.0,
        background: false,
    }
}
fn submit(core: &mut RuntimeCore, operation: SubmitJob) -> JobReceipt {
    let connection = format!("cli-{}", uuid::Uuid::new_v4());
    let cli = open_on(core, &connection, "cli");
    let outcome = core
        .handle(&connection, request(Call::Submit(operation), Some(cli)))
        .outcome;
    let Outcome::Receipt(receipt) = outcome else {
        panic!("{outcome:?}");
    };
    receipt
}
fn row(core: &mut RuntimeCore, run: &str) -> Value {
    core.with_tool_fixture_store(|store| {
        let text: String = store
            .connection
            .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [run], |r| {
                r.get(0)
            })
            .unwrap();
        assert!(!text.contains("PRIVATE_TOOL_JOB_INPUT_CANARY"));
        serde_json::from_str(&text).unwrap()
    })
}
// This entire test module is Windows-only. Clear the DOS readonly attribute so
// a rejected write open proves the held sharing mode, rather than that bit.
#[allow(clippy::permissions_set_readonly_false)]
fn writable(path: &Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(false);
    fs::set_permissions(path, permissions).unwrap();
}
fn readonly(path: &Path) {
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_readonly(true);
    fs::set_permissions(path, permissions).unwrap();
}
fn write_handle(path: &Path) -> std::io::Result<fs::File> {
    fs::OpenOptions::new().write(true).open(path)
}

#[test]
fn default_production_review_does_not_accept_a_fixture_job_or_change_the_tool_inventory() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let before = world.metadata(KEY);
    let replacement = world.fixture.trust();
    let trust = std::mem::replace(&mut world.trust, replacement);
    let result = RuntimeCore::managed_tool_catalog(
        tokens(),
        world.store.take().unwrap(),
        false,
        false,
        catalog,
        trust,
    );
    assert!(matches!(result, Err(ErrorCode::DependencyArtifactMismatch)));
    world.reopen();
    assert_eq!(world.metadata(KEY), before);
    assert_eq!(
        world
            .store
            .as_ref()
            .unwrap()
            .connection
            .query_row("SELECT count(*) FROM jobs", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        0
    );
}
#[test]
fn accepted_job_never_implicitly_materializes_a_missing_image() {
    let mut world = World::new();
    world.install().unwrap();
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let cli = open(&mut core, "cli");
    let outcome = core
        .handle(
            "cli",
            request(Call::Submit(operation("consumer", "missing")), Some(cli)),
        )
        .outcome;
    assert!(
        matches!(
            outcome,
            Outcome::Error(RuntimeError {
                code: ErrorCode::DependencyArtifactMismatch
            })
        ),
        "{outcome:?}"
    );
    assert!(!world.path.join("tool-image-store-v1").exists());
    core.with_tool_fixture_store(|store| {
        assert_eq!(store.connection.query_row("SELECT count(*) FROM jobs", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
        assert!(!store.connection.query_row("SELECT EXISTS(SELECT 1 FROM core_metadata WHERE key='tool-image-materialization-pending-v1')", [], |r| r.get::<_,bool>(0)).unwrap());
    });
}
#[test]
fn two_queued_consumers_hold_one_native_image_until_both_durable_cancellations() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let path = image.join("bin/ffmpeg.exe");
    let catalog = catalog(&world, &["consumer", "middle"]);
    let mut core = core(&mut world, catalog.clone(), false);
    for id in ["consumer", "middle"] {
        grant(&mut core, &catalog, id);
    }
    let first = submit(&mut core, operation("consumer", "first"));
    let second = submit(&mut core, operation("middle", "second"));
    let first_row = row(&mut core, &first.run_id);
    let second_row = row(&mut core, &second.run_id);
    assert_eq!(
        first_row["toolArtifacts"]["images"],
        second_row["toolArtifacts"]["images"]
    );
    assert_eq!(
        first_row["toolArtifacts"]["profile"],
        "accepted-t1-tool-artifacts-v1"
    );
    assert_eq!(first_row["toolArtifacts"]["retained"], true);
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/artifacts"))
            .unwrap()
            .count(),
        1
    );
    writable(&path);
    assert!(write_handle(&path).is_err());
    assert!(fs::rename(&image, image.with_extension("moved")).is_err());
    core.cancel(&first.run_id);
    assert_eq!(core.state(&first.run_id), Some(JobState::Cancelled));
    assert_eq!(
        row(&mut core, &first.run_id)["toolArtifacts"]["retained"],
        false
    );
    assert!(write_handle(&path).is_err());
    core.cancel(&second.run_id);
    assert_eq!(
        row(&mut core, &second.run_id)["toolArtifacts"]["retained"],
        false
    );
    drop(write_handle(&path).unwrap());
    readonly(&path);
    assert_eq!(
        fs::read(&path).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
}
#[test]
fn a_lease_does_not_grant_tool_execution_and_durable_completion_releases_handles() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let path = image.join("bin/ffmpeg.exe");
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "running"));
    assert!(core.take_run(&receipt.run_id).is_some());
    let lock = row(&mut core, &receipt.run_id)["snapshot"]["dependencyLock"]
        .as_str()
        .unwrap()
        .to_owned();
    assert!(core
        .invoke_service::<()>(
            &receipt.run_id,
            &CapabilityOperation::ToolExecute {
                lock,
                action: "inspect-v1".into()
            },
            |_| panic!("A retention lease cannot invoke a tool adapter")
        )
        .is_err());
    let mut forged = serde_json::to_value(operation("consumer", "forged")).unwrap();
    forged["toolArtifacts"] = row(&mut core, &receipt.run_id)["toolArtifacts"].clone();
    assert!(serde_json::from_value::<SubmitJob>(forged).is_err());
    core.finish(&receipt.run_id, Ok(json!("result")));
    assert_eq!(core.state(&receipt.run_id), Some(JobState::Succeeded));
    assert_eq!(
        row(&mut core, &receipt.run_id)["toolArtifacts"]["retained"],
        false
    );
    writable(&path);
    drop(write_handle(&path).unwrap());
    readonly(&path);
}
#[test]
fn a_queued_job_reopens_with_the_same_seal_and_idempotency_key_without_automatic_execution() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let path = image.join("bin/ffmpeg.exe");
    let catalog = catalog(&world, &["consumer"]);
    let mut first_core = core(&mut world, catalog.clone(), false);
    grant(&mut first_core, &catalog, "consumer");
    let mut request = operation("consumer", "reopen");
    request.background = true;
    let receipt = submit(&mut first_core, request.clone());
    let before = row(&mut first_core, &receipt.run_id)["toolArtifacts"].clone();
    drop(first_core);
    world.reopen();
    let mut manager = core(&mut world, catalog.clone(), true);
    assert!(manager.pending_runs().is_empty());
    assert_eq!(manager.state(&receipt.run_id), Some(JobState::Queued));
    assert_eq!(row(&mut manager, &receipt.run_id)["toolArtifacts"], before);
    drop(manager);
    world.reopen();
    let mut reopened = core(&mut world, catalog, false);
    assert_eq!(reopened.pending_runs(), vec![receipt.run_id.clone()]);
    assert_eq!(row(&mut reopened, &receipt.run_id)["toolArtifacts"], before);
    assert_eq!(submit(&mut reopened, request).run_id, receipt.run_id);
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
    reopened.cancel(&receipt.run_id);
    assert_eq!(
        row(&mut reopened, &receipt.run_id)["toolArtifacts"]["retained"],
        false
    );
}
