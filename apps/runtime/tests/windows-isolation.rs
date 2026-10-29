#![cfg(windows)]
#[path = "isolation/filesystem.rs"]
mod filesystem;
#[path = "isolation/harness.rs"]
mod harness;
use harness::*;
use std::{
    fs,
    net::TcpListener,
    time::{Duration, Instant},
};

fn report(fixture: &Fixture, child: &Child) -> serde_json::Value {
    let (code, _stdout, stderr) = child.output(Duration::from_secs(5), 65536).unwrap();
    assert_eq!(
        code,
        0,
        "native fixture exit {code:#x}: {}",
        String::from_utf8_lossy(&stderr)
    );
    let bytes = fs::read(fixture.work.join("report.json")).unwrap();
    assert!(bytes.len() < 16384);
    let report: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(report["pid"], child.pub_pid);
    child.wait_empty(Duration::from_millis(500)).unwrap();
    report
}
#[test]
fn real_lpac_file_network_and_loader_denials_have_an_uncontained_positive_control() {
    let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let fixture = Fixture::new().unwrap();
    filesystem::junction(&fixture.work.join("redirect"), &fixture.host).unwrap();
    assert_eq!(
        fs::read(fixture.work.join("redirect/private.txt")).unwrap(),
        b"HOST_PRIVATE_CANARY"
    );
    let child = fixture
        .launch(false, "probe", port, 512 * 1024 * 1024)
        .unwrap();
    let control = report(&fixture, &child);
    assert_eq!(control["hostRead"], true);
    assert_eq!(control["hostWrite"], true);
    assert_eq!(control["network"], true);
    assert_eq!(control["lowDllAllowed"], true);
    assert_eq!(control["memoryAllowed"], true);
    assert_eq!(control["registryStatus"], 0);
    assert!(child.private_inherited);
    assert_eq!(control["otherRead"], true);
    assert_eq!(control["otherWrite"], true);
    assert_eq!(control["junctionRead"], true);
    assert_eq!(control["junctionWrite"], true);
    assert_eq!(control["ipc"], true);
    drop(child);
    fs::remove_file(fixture.work.join("report.json")).unwrap();
    fs::remove_file(fixture.host.join("escape.txt")).unwrap();
    fs::remove_file(fixture.host.join("redirected.txt")).unwrap();
    fs::remove_file(fixture.root.join("other-task/escape.txt")).unwrap();
    let child = fixture
        .launch(true, "probe", port, 128 * 1024 * 1024)
        .unwrap();
    let confined = report(&fixture, &child);
    assert_eq!(confined["container"], 1);
    assert_eq!(confined["lpac"], true);
    assert_eq!(confined["capabilities"], 1);
    assert_eq!(confined["sid"], fixture.container.text);
    assert_eq!(confined["registryStatus"], 5);
    assert!(!child.private_inherited);
    assert_eq!(confined["otherRead"], false);
    assert_eq!(confined["otherWrite"], false);
    assert_eq!(confined["junctionRead"], false);
    assert_eq!(confined["junctionWrite"], false);
    assert_eq!(confined["ipc"], false);
    assert_eq!(confined["ipcError"], 5);
    assert_eq!(confined["hostRead"], false);
    assert_eq!(confined["hostWrite"], false);
    assert_eq!(confined["workspaceWrite"], true);
    assert_eq!(confined["network"], false);
    assert_eq!(confined["networkError"], 10013);
    assert_eq!(confined["memoryAllowed"], false);
    assert_eq!(confined["lowDllAllowed"], false);
    assert_eq!(confined["systemDll"], true);
    assert_eq!(confined["hostEnvironment"], false);
    assert_eq!(
        fs::read(fixture.host.join("private.txt")).unwrap(),
        b"HOST_PRIVATE_CANARY"
    );
    assert!(!fixture.host.join("escape.txt").exists());
    assert!(!fixture.root.join("other-task/escape.txt").exists());
    assert!(!fixture.host.join("redirected.txt").exists());
    fs::remove_dir(fixture.work.join("redirect")).unwrap();
}

#[test]
fn admission_refusal_reaps_the_suspended_process_without_running_it() {
    let fixture = Fixture::new().unwrap();
    let child = fixture.refused_launch().unwrap();
    assert!(child.exited().unwrap());
    assert_eq!(child.active().unwrap(), 0);
    assert_eq!(child.wait(Duration::from_millis(500)).unwrap(), 71);
    assert!(!fixture.work.join("ready").exists());
    assert!(!fixture.work.join("report.json").exists());
}

#[test]
fn child_creation_is_blocked_before_the_descendant_runs() {
    let fixture = Fixture::new().unwrap();
    let child = fixture
        .launch(false, "child", 1, 128 * 1024 * 1024)
        .unwrap();
    let control = report(&fixture, &child);
    assert_eq!(control["childCreated"], true);
    assert_eq!(child.active_limit, 2);
    assert_eq!(child.child_policy, 0);
    assert!(fixture.work.join("grandchild-ran").is_file());
    drop(child);
    fs::remove_file(fixture.work.join("grandchild-ran")).unwrap();
    fs::remove_file(fixture.work.join("report.json")).unwrap();
    let child = fixture.launch(true, "child", 1, 128 * 1024 * 1024).unwrap();
    let confined = report(&fixture, &child);
    assert_eq!(confined["childCreated"], false);
    assert_eq!(child.active_limit, 2);
    assert_eq!(child.child_policy, 1); // NoChildProcessCreation, no audit/override.
    assert!(
        matches!(confined["childError"].as_u64(), Some(5 | 367)),
        "only observed child-creation denials are accepted: {confined}"
    );
    assert!(!fixture.work.join("grandchild-ran").exists());
}

#[test]
fn output_flood_is_bounded_and_the_actual_job_is_reaped() {
    let fixture = Fixture::new().unwrap();
    let child = fixture.launch(true, "flood", 1, 128 * 1024 * 1024).unwrap();
    let error = child.output(Duration::from_secs(5), 65536).unwrap_err();
    assert_eq!(error.to_string(), "output budget exceeded");
    assert!(fixture.work.join("ready").is_file());
    assert!(child.exited().unwrap());
    assert_eq!(child.wait(Duration::from_millis(500)).unwrap(), 71);
    assert_eq!(child.active().unwrap(), 0);
    assert!(child.started.elapsed() < Duration::from_secs(5));
}
#[test]
fn an_ignoring_native_loop_is_killed_by_its_owned_job_within_the_deadline_budget() {
    let fixture = Fixture::new().unwrap();
    let child = fixture.launch(true, "spin", 1, 128 * 1024 * 1024).unwrap();
    let deadline = child.started + Duration::from_millis(500);
    while Instant::now() < deadline {
        assert!(!child.exited().unwrap());
        std::thread::sleep(Duration::from_millis(5));
    }
    assert!(
        fixture.work.join("ready").exists(),
        "actual child must have run before cancellation"
    );
    child.terminate().unwrap();
    assert_eq!(child.wait(Duration::from_millis(500)).unwrap(), 71);
    assert!(child.started.elapsed() <= Duration::from_millis(1000));
    assert_eq!(child.active().unwrap(), 0);
}
#[test]
fn closing_the_owned_job_terminates_the_actual_lpac_process() {
    let fixture = Fixture::new().unwrap();
    let mut child = fixture.launch(true, "spin", 1, 128 * 1024 * 1024).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !fixture.work.join("ready").exists() {
        assert!(!child.exited().unwrap());
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(child.active().unwrap(), 1);
    assert!(!child.exited().unwrap());
    drop(child.job.take());
    child.wait(Duration::from_millis(500)).unwrap();
    assert!(child.exited().unwrap());
}
