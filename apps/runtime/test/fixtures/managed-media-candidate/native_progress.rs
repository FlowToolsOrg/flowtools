//! Real native conversion stages, current authorization and owned drain.
use super::*;
use crate::runtime::{MediaAudioState, MediaConversionStage as Stage, MediaVideoState};
use std::time::Instant;

fn wait(started: Instant) {
    assert!(started.elapsed() < Duration::from_secs(10));
    std::thread::sleep(Duration::from_millis(1));
}
fn drained(world: &World, run: &str) {
    let seen = world.core.media_audio_fixture_observations(run);
    assert_eq!(seen.len(), 2);
    for (pid, active, exited, _, _) in seen {
        assert_ne!(pid, 0);
        assert_eq!(active, 0);
        assert!(exited);
    }
}
#[test]
fn wav_progress_observes_conversion_validation_and_complete_without_collecting_or_charging_again() {
    let mut world = World::new();
    world.grant_limit("consumer", 1);
    let run = world.start_granted("consumer");
    let handle = world.core.start_media_audio(&run, &wave()).unwrap();
    assert_eq!(
        world.core.media_audio_progress(&handle).unwrap().stage,
        Stage::Converting
    );
    let started = Instant::now();
    let mut validating = false;
    loop {
        world.core.tick_media_inspections().unwrap();
        let progress = world.core.media_audio_progress(&handle).unwrap();
        validating |= progress.stage == Stage::Validating;
        if progress.stage == Stage::Complete {
            assert!(validating);
            assert!(progress.advisory.ended);
            let row = world.row(&run);
            assert_eq!(world.core.media_audio_progress(&handle).unwrap(), progress);
            assert_eq!(world.row(&run), row);
            assert_eq!(row["nativeAudio"].as_array().unwrap().len(), 1);
            break;
        }
        wait(started);
    }
    drained(&world, &run);
    assert!(matches!(
        world.core.poll_media_audio(&handle).unwrap(),
        MediaAudioState::Complete(_)
    ));
    assert_eq!(
        world.core.media_audio_progress(&handle).err(),
        Some(ErrorCode::SessionInvalid)
    );
    assert_eq!(
        world.core.start_media_audio(&run, &wave()).err(),
        Some(ErrorCode::BudgetExceeded)
    );
    assert_eq!(world.row(&run)["nativeAudio"].as_array().unwrap().len(), 1);
}
#[test]
fn webm_progress_does_not_turn_advisory_time_into_a_duration_or_validation_success() {
    let mut world = World::new();
    let run = world.start("consumer");
    let handle = world
        .core
        .start_media_video(&run, &video_input::video(3, 16))
        .unwrap();
    assert_eq!(
        world.core.media_video_progress(&handle).unwrap().stage,
        Stage::Converting
    );
    let started = Instant::now();
    let mut validating = false;
    loop {
        world.core.tick_media_inspections().unwrap();
        let progress = world.core.media_video_progress(&handle).unwrap();
        if progress.stage == Stage::Validating {
            validating = true;
            assert!(progress.advisory.ended);
            assert_eq!(world.row(&run)["nativeVideo"][0]["phase"], "probing");
        }
        if progress.stage == Stage::Complete {
            break;
        }
        wait(started);
    }
    assert!(validating);
    drained(&world, &run);
    let MediaVideoState::Complete(output) = world.core.poll_media_video(&handle).unwrap() else {
        panic!("Expected video")
    };
    assert_eq!(output.metadata().duration_ms, None);
    assert_eq!(output.decoded_frames(), &[(0, 3)]);
}
#[test]
fn progress_refuses_a_foreign_core_handle_without_touching_its_tasks() {
    let mut first = World::new();
    let run = first.start("consumer");
    let handle = first.core.start_media_audio(&run, &wave()).unwrap();
    let mut other = World::new();
    let second = other.start("consumer");
    let before = other.row(&second);
    assert_eq!(
        other.core.media_audio_progress(&handle).err(),
        Some(ErrorCode::InstanceMismatch)
    );
    assert_eq!(other.row(&second), before);
    first.core.finish(&run, Err(ErrorCode::Aborted));
}
#[test]
fn revoked_progress_observation_drains_the_real_process_and_never_returns_a_stale_snapshot() {
    let mut world = World::new();
    let run = world.start("consumer");
    let handle = world.core.start_media_audio(&run, &wave()).unwrap();
    let admin = world.open("admin");
    super::native_inspection::revoke(&mut world.core, admin);
    assert_eq!(
        world.core.media_audio_progress(&handle).err(),
        Some(ErrorCode::GrantRevoked)
    );
    for (pid, active, exited, _, _) in world.core.media_audio_fixture_observations(&run) {
        assert_ne!(pid, 0);
        assert_eq!(active, 0);
        assert!(exited);
    }
    assert!(world.core.poll_media_audio(&handle).is_err());
}
#[test]
fn substituted_job_metadata_refuses_progress_and_retains_the_original_native_input_and_receipt() {
    let mut world = World::new();
    let run = world.start("consumer");
    let handle = world.core.start_media_audio(&run, &wave()).unwrap();
    let id = world.row(&run)["nativeAudio"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .execute(
                "UPDATE jobs SET metadata='FOREIGN_PROGRESS_ROW' WHERE run_id=?1",
                [&run],
            )
            .unwrap();
    });
    assert_eq!(
        world.core.media_audio_progress(&handle).err(),
        Some(ErrorCode::StoreCorrupt)
    );
    let raw = world.core.with_tool_fixture_store(|store| {
        store
            .connection
            .query_row("SELECT metadata FROM jobs WHERE run_id=?1", [&run], |r| {
                r.get::<_, String>(0)
            })
            .unwrap()
    });
    assert_eq!(raw, "FOREIGN_PROGRESS_ROW");
    assert!(world
        .path
        .join("native-media-inspection-v1")
        .join(id)
        .join("input")
        .is_file());
    let seen = world.core.media_audio_fixture_observations(&run);
    assert!(!seen.is_empty());
    assert!(seen
        .iter()
        .all(|(_, active, exited, _, _)| *active == 0 && *exited));
}
