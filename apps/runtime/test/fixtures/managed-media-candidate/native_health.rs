//! Fixed synthetic native health, not caller input or release activation.
use super::*;
use crate::runtime::{ToolHealthHandle, ToolHealthObservation, ToolHealthState};
use std::time::Instant;

fn completed(
    core: &mut RuntimeCore,
    handle: &ToolHealthHandle,
    run: &str,
) -> ToolHealthObservation {
    settled(core, run);
    drained(core, run);
    let ToolHealthState::Healthy(observed) = core.poll_tool_health(handle).unwrap() else {
        panic!("Expected completed fixed health observation");
    };
    observed
}
fn settled(core: &mut RuntimeCore, world_run: &str) {
    let started = Instant::now();
    while core.with_tool_fixture_store(|store| {
        let text: String = store
            .connection
            .query_row(
                "SELECT metadata FROM jobs WHERE run_id=?1",
                [world_run],
                |row| row.get(0),
            )
            .unwrap();
        let value: Value = serde_json::from_str(&text).unwrap();
        value["nativeAudio"][0]["phase"] != "completed"
    }) {
        assert!(started.elapsed() < Duration::from_secs(10));
        core.tick_media_inspections().unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
}
fn drained(core: &RuntimeCore, run: &str) {
    let observations = core.media_audio_fixture_observations(run);
    assert_eq!(observations.len(), 2);
    for observed in observations {
        assert_ne!(observed.0, 0);
        assert_eq!(observed.1, 0);
        assert!(observed.2);
    }
}

#[test]
fn fixed_health_runs_both_real_owned_tools_and_consumes_one_normal_call() {
    let mut world = World::new();
    world.grant_options("consumer", 1, false);
    let run = world.start_granted("consumer");
    let started = now();
    let handle = world.core.start_tool_health(&run).unwrap();
    let observed = completed(&mut world.core, &handle, &run);
    assert_eq!(
        observed.pin().archive_digest,
        world.fixture.package.archive.sha256
    );
    assert_eq!(observed.pin().version, world.fixture.package.version);
    assert!(observed.checked_at_ms() >= started && observed.checked_at_ms() <= now());
    assert_eq!(world.row(&run)["nativeAudio"].as_array().unwrap().len(), 1);
    assert_eq!(
        world.core.poll_tool_health(&handle).err(),
        Some(ErrorCode::SessionInvalid)
    );
    assert_eq!(
        world.core.start_tool_health(&run).err(),
        Some(ErrorCode::BudgetExceeded)
    );
    world.core.finish(&run, Err(ErrorCode::Aborted));
    assert_eq!(world.row(&run)["toolArtifacts"]["retained"], false);
}

#[test]
fn an_old_inflight_roundtrip_cannot_report_current_health_after_a_floor_advance() {
    let mut world = World::new();
    let release = super::native_continuation::newer(&world);
    let trust = world.fixture.trust();
    let clock = world.fixture.clock();
    let run = world.start("consumer");
    let handle = world.core.start_tool_health(&run).unwrap();
    super::native_continuation::advance(&mut world.core, &trust, &release, clock);
    settled(&mut world.core, &run);
    drained(&world.core, &run);
    assert_eq!(
        world.core.poll_tool_health(&handle).err(),
        Some(ErrorCode::DependencyArtifactMismatch)
    );
    world.core.finish(&run, Err(ErrorCode::Aborted));
}

#[test]
fn revocation_after_native_completion_refuses_health_at_collection() {
    let mut world = World::new();
    let run = world.start("consumer");
    let admin = world.open("admin");
    let handle = world.core.start_tool_health(&run).unwrap();
    settled(&mut world.core, &run);
    drained(&world.core, &run);
    super::native_inspection::revoke(&mut world.core, admin);
    assert_eq!(
        world.core.poll_tool_health(&handle).err(),
        Some(ErrorCode::GrantRevoked)
    );
}

#[test]
fn cancelling_health_does_not_revoke_the_other_shared_consumer() {
    let mut world = World::new();
    let a = world.start("consumer");
    let b = world.start("other-consumer");
    let first = world.core.start_tool_health(&a).unwrap();
    let second = world.core.start_tool_health(&b).unwrap();
    world.core.cancel(&a);
    assert_eq!(
        world.core.poll_tool_health(&first).err(),
        Some(ErrorCode::Aborted)
    );
    let observed = completed(&mut world.core, &second, &b);
    assert_eq!(
        observed.pin().archive_digest,
        world.fixture.package.archive.sha256
    );
    world.core.finish(&b, Err(ErrorCode::Aborted));
}

#[test]
fn a_health_handle_is_bound_to_its_original_core_and_cannot_be_forged_from_a_pin() {
    let mut first = World::new();
    let mut second = World::new();
    let run = first.start("consumer");
    let handle = first.core.start_tool_health(&run).unwrap();
    assert_eq!(
        second.core.poll_tool_health(&handle).err(),
        Some(ErrorCode::InstanceMismatch)
    );
    assert_eq!(second.core.active_media_inspections(), 0);
    completed(&mut first.core, &handle, &run);
    first.core.finish(&run, Err(ErrorCode::Aborted));
}
