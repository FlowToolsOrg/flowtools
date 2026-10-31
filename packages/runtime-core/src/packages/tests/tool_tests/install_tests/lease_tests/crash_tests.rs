use super::*;
use crate::packages::tools::install::leases::{exit_fixture_at, FixtureExit};
use std::time::{Duration, Instant};
const PROFILE_ENV: &str = "FLOWTOOLS_ACCEPTED_TOOL_CRASH_PROFILE";
const POINT_ENV: &str = "FLOWTOOLS_ACCEPTED_TOOL_CRASH_POINT";

#[test]
fn accepted_tool_crash_child() {
    let Some(profile) = std::env::var_os(PROFILE_ENV) else {
        return;
    };
    let profile = PathBuf::from(profile);
    assert_eq!(profile.parent().unwrap(), std::env::temp_dir());
    assert!(profile
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("flowtools-tool-install-")
        .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| id.to_string() == s)));
    let mut world = World::open(&profile, false, false);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let point = std::env::var(POINT_ENV).unwrap();
    match point.as_str() {
        "before-accept-commit" => exit_fixture_at(FixtureExit::BeforeAcceptCommit),
        "accept-committed" => exit_fixture_at(FixtureExit::AcceptCommitted),
        "terminal-committed" => exit_fixture_at(FixtureExit::TerminalCommitted),
        "release-committed" => exit_fixture_at(FixtureExit::ReleaseCommitted),
        "queued" | "running" => {}
        _ => panic!("Unknown crash fixture point"),
    }
    let receipt = submit(&mut core, operation("consumer", "crash"));
    if point == "queued" {
        std::process::exit(85);
    }
    assert!(core.take_run(&receipt.run_id).is_some());
    if point == "running" {
        std::process::exit(85);
    }
    core.finish(&receipt.run_id, Ok(json!("fixture")));
    panic!("The configured SQL barrier must exit this actual child");
}
#[test]
fn actual_process_exits_preserve_atomic_job_lease_rows_and_all_native_source_bytes() {
    for point in [
        "before-accept-commit",
        "accept-committed",
        "queued",
        "running",
        "terminal-committed",
        "release-committed",
    ] {
        let mut world = World::new();
        let image = prepare_image(&mut world);
        let raw = world.metadata(KEY).unwrap();
        let image_inventory = world.metadata("tool-image-inventory-v1").unwrap();
        drop(world.store.take());
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "packages::tests::tool_tests::install_tests::lease_tests::crash_tests::accepted_tool_crash_child", "--nocapture"])
            .env(PROFILE_ENV, &world.path).env(POINT_ENV, point).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("Actual owned child missed its 20s bound at {point}");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(85), "{point}");
        world.reopen();
        assert_eq!(
            world.metadata(KEY).as_deref(),
            Some(raw.as_str()),
            "{point}"
        );
        assert_eq!(
            world.metadata("tool-image-inventory-v1").as_deref(),
            Some(image_inventory.as_str()),
            "{point}"
        );
        assert_eq!(
            fs::read(image.join("bin/ffmpeg.exe")).unwrap(),
            world.fixture.contents["bin/ffmpeg.exe"]
        );
        let rows = world.store.as_ref().unwrap().load_jobs().unwrap();
        if point == "before-accept-commit" {
            assert!(rows.is_empty());
            let inputs = fs::read_dir(world.path.join("private-jobs"))
                .unwrap()
                .map(|entry| entry.unwrap().path())
                .filter(|path| {
                    path.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .ends_with(".input.bin")
                })
                .collect::<Vec<_>>();
            assert_eq!(inputs.len(), 1, "{point}");
            assert!(!fs::read(&inputs[0])
                .unwrap()
                .windows(b"PRIVATE_TOOL_JOB_INPUT_CANARY".len())
                .any(|bytes| bytes == b"PRIVATE_TOOL_JOB_INPUT_CANARY"));
        } else {
            assert_eq!(rows.len(), 1);
            let row = &rows[0];
            assert_eq!(
                row["toolArtifacts"]["retained"],
                point != "release-committed",
                "{point}"
            );
            let expected = match point {
                "accept-committed" => "accepted",
                "queued" => "queued",
                "running" => "running",
                _ => "succeeded",
            };
            assert_eq!(row["snapshot"]["state"], expected, "{point}");
            assert!(!serde_json::to_string(row)
                .unwrap()
                .contains("PRIVATE_TOOL_JOB_INPUT_CANARY"));
            let id = row["snapshot"]["runId"].as_str().unwrap();
            assert!(
                world
                    .path
                    .join("private-jobs")
                    .join(format!("{id}.input.bin"))
                    .is_file(),
                "{point}"
            );
        }
        // All roots refer to the original known ledger and exact seals even
        // when the process disappeared before/after the durable ack boundary.
        assert_eq!(world.pins().unwrap().len(), 1, "{point}");
    }
}
