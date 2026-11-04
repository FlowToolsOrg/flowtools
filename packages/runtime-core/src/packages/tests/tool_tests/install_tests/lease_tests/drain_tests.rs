use super::super::image_tests::harness;
use super::*;
use crate::services::ServiceTarget;
use std::time::Duration;

#[test]
fn root_failure_keeps_tool_handles_until_the_actual_owned_service_job_drains() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let path = image.join("bin/ffmpeg.exe");
    let mut manifests = catalog(&world, &["consumer", "middle"])
        .list()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    manifests
        .iter_mut()
        .find(|value| value["id"] == "consumer")
        .unwrap()["dependencies"]["services"] = json!([{
        "publisher":"fixture-publisher", "id":"middle", "version":"^1.0.0", "service":"text-transform", "interfaceVersion":"^1.0.0",
    }]);
    let catalog = BuiltinCatalog::from_host_manifests(manifests).unwrap();
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    grant_command(&mut core, &catalog, "middle", "svc:text-transform:convert");
    let receipt = submit(&mut core, operation("consumer", "owned-drain"));
    assert!(core.take_run(&receipt.run_id).is_some());
    let service = core
        .prepare_service(
            &receipt.run_id,
            ServiceTarget {
                publisher: "fixture-publisher".into(),
                id: "middle".into(),
                service: "text-transform".into(),
                operation: "convert".into(),
            },
            json!({"text":"service input"}),
        )
        .unwrap();
    core.start_service(&service.spec.run_id).unwrap();
    let fixture = harness::Fixture::new().unwrap();
    let child = fixture
        .launch(true, "spin", 1, 128 * 1024 * 1024, &[])
        .unwrap();
    assert!(child.pub_pid > 0);
    assert_eq!(child.active().unwrap(), 1);
    assert!(!child.exited().unwrap());
    // The ordinary T0 completion boundary must retain every artifact while an
    // actual service process still owns its live, accounted Job.
    core.finish(&receipt.run_id, Err(ErrorCode::ExecutionFailed));
    assert_eq!(core.state(&receipt.run_id), Some(JobState::Failed));
    assert_eq!(core.active_service_calls(), 1);
    assert_eq!(
        row(&mut core, &receipt.run_id)["toolArtifacts"]["retained"],
        true
    );
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
    assert_eq!(child.active().unwrap(), 1);
    assert!(!child.exited().unwrap());
    child.terminate().unwrap();
    assert_eq!(child.wait(Duration::from_millis(500)).unwrap(), 71);
    child.wait_empty(Duration::from_millis(500)).unwrap();
    assert!(child.exited().unwrap());
    assert_eq!(child.active().unwrap(), 0);
    assert!(core
        .finish_service(&service.spec.run_id, Err(ErrorCode::Aborted))
        .is_err());
    assert_eq!(core.active_service_calls(), 0);
    assert_eq!(
        row(&mut core, &receipt.run_id)["toolArtifacts"]["retained"],
        false
    );
    writable(&path);
    drop(write_handle(&path).unwrap());
    readonly(&path);
    assert_eq!(
        fs::read(&path).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
}
