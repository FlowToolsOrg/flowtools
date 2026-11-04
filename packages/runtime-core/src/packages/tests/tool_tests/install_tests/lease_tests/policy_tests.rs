use super::*;
use crate::packages::{tools::install::images::materialize_fixture_with_hook, RootPayload};

#[test]
fn revoke_then_regrant_cannot_dispatch_an_old_accepted_epoch_or_replace_its_receipt() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let operation = operation("consumer", "epoch");
    let receipt = submit(&mut core, operation.clone());
    let old = row(&mut core, &receipt.run_id);
    let admin = open(&mut core, "admin");
    let outcome = core
        .handle(
            "admin",
            request(
                Call::Revoke(PermissionKey {
                    plugin_id: "consumer".into(),
                    command_id: "convert".into(),
                    target: GrantTarget::Cli,
                }),
                Some(admin),
            ),
        )
        .outcome;
    assert!(matches!(outcome, Outcome::Permissions(_)), "{outcome:?}");
    core.disconnect("admin");
    grant(&mut core, &catalog, "consumer");
    assert!(core.take_run(&receipt.run_id).is_none());
    assert_eq!(core.state(&receipt.run_id), Some(JobState::Failed));
    assert_eq!(
        row(&mut core, &receipt.run_id)["snapshot"]["grantEpoch"],
        old["snapshot"]["grantEpoch"]
    );
    assert_eq!(
        row(&mut core, &receipt.run_id)["toolArtifacts"]["retained"],
        false
    );
    assert_eq!(submit(&mut core, operation).run_id, receipt.run_id);
    assert!(core.pending_runs().is_empty());
    let next = submit(&mut core, super::operation("consumer", "new-epoch"));
    assert_ne!(
        row(&mut core, &next.run_id)["snapshot"]["grantEpoch"],
        old["snapshot"]["grantEpoch"]
    );
    core.cancel(&next.run_id);
}
#[test]
fn current_signed_root_revocation_interrupts_dispatch_and_preserves_the_exact_old_roots() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "root-revoke"));
    let accepted = row(&mut core, &receipt.run_id)["toolArtifacts"].clone();
    let envelope = serde_json::to_vec(&world.fixture.base.golden["rootEnvelope"]).unwrap();
    let mut root: RootPayload = serde_json::from_slice(
        &decode_envelope(&envelope, ROOT_PAYLOAD_TYPE)
            .unwrap()
            .payload,
    )
    .unwrap();
    root.version += 1;
    root.previous_root_sha256 = Some(
        core.with_tool_fixture_store(|store| world.trust.snapshot(store).unwrap().root_digest),
    );
    root.revoked_package_digests
        .push(world.fixture.base.package.archive.sha256.clone());
    core.with_tool_fixture_store(|store| {
        world
            .trust
            .rotate(
                store,
                &signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]),
                world.fixture.now(),
            )
            .unwrap()
    });
    assert!(core.take_run(&receipt.run_id).is_none());
    assert_eq!(core.state(&receipt.run_id), Some(JobState::Interrupted));
    assert_eq!(row(&mut core, &receipt.run_id)["toolArtifacts"], accepted);
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
    assert!(world
        .path
        .join("private-jobs")
        .join(format!("{}.input.bin", receipt.run_id))
        .is_file());
}
#[test]
fn a_new_materialized_version_coexists_with_held_old_jobs_without_resetting_release_floors() {
    let mut world = World::new();
    let old_image = prepare_image(&mut world);
    let catalog = catalog(&world, &["consumer"]);
    let mut core = core(&mut world, catalog.clone(), false);
    grant(&mut core, &catalog, "consumer");
    let receipt = submit(&mut core, operation("consumer", "old-version"));
    let accepted = row(&mut core, &receipt.run_id)["toolArtifacts"].clone();
    let old_pin = world.fixture.base.package.archive.sha256.clone();
    let old_bytes = world.fixture.contents["bin/ffmpeg.exe"].clone();
    world.fixture.base.package.version = "1.0.1".into();
    world.fixture.base.package.release_sequence += 1;
    world.fixture.manifest["version"] = json!("1.0.1");
    world.fixture.contents.insert(
        "bin/ffmpeg.exe".into(),
        b"NEW_INERT_TOOL_CANARY_NOT_EXECUTABLE".to_vec(),
    );
    world.fixture.rebuild();
    let envelope = signed(PACKAGE_PAYLOAD_TYPE, &world.fixture.base.package, &[3]);
    let source = core.with_tool_fixture_store(|store| {
        install_fixture_with_hook(
            &world.trust,
            store,
            world.fixture.input(&envelope),
            || world.fixture.now(),
            |_, _| Ok(()),
        )
        .unwrap()
    });
    let new = core.with_tool_fixture_store(|store| {
        materialize_fixture_with_hook(
            &world.trust,
            store,
            &source,
            || world.fixture.now(),
            |_, _| Ok(()),
        )
        .unwrap()
    });
    assert_ne!(new.pin().archive_digest, old_pin);
    drop(new);
    let new_image = world
        .path
        .join("tool-image-store-v1/artifacts")
        .join(&world.fixture.base.package.archive.sha256);
    assert_eq!(
        fs::read(new_image.join("bin/ffmpeg.exe")).unwrap(),
        world.fixture.contents["bin/ffmpeg.exe"]
    );
    assert_eq!(
        fs::read(old_image.join("bin/ffmpeg.exe")).unwrap(),
        old_bytes
    );
    let path = old_image.join("bin/ffmpeg.exe");
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
    // Ordinary signed admission still refuses the now-old release. Accepted
    // continuation is a separate future protocol; never lower a trust floor.
    assert!(core.take_run(&receipt.run_id).is_none());
    assert_eq!(core.state(&receipt.run_id), Some(JobState::Interrupted));
    assert_eq!(row(&mut core, &receipt.run_id)["toolArtifacts"], accepted);
    core.with_tool_fixture_store(|store| {
        assert_eq!(inspect_inert_tools(&world.trust, store).unwrap().len(), 2)
    });
    assert_eq!(
        fs::read_dir(world.path.join("tool-image-store-v1/artifacts"))
            .unwrap()
            .count(),
        2
    );
}
