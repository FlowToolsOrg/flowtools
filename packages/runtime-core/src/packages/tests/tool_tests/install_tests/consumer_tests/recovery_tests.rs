use super::super::reconciliation_tests::{authority, recover};
use super::*;
use std::{
    cell::Cell,
    time::{Duration, Instant},
};

#[test]
fn existing_provider_transitive_pins_cannot_silently_change_when_a_new_consumer_installs() {
    let mut world = world();
    world.install().unwrap();
    let provider = input(&world, "provider", vec![], false, 1);
    let compatibility = compatibility(&world);
    let now = world.fixture.now();
    install::install_inert_plugin(
        &world.trust,
        world.store.as_mut().unwrap(),
        provider.borrow(),
        &compatibility,
        || now,
    )
    .unwrap();
    let middle = input(&world, "middle", vec![service("provider")], true, 1);
    install(&mut world, &middle).unwrap();
    let before = world.metadata(CONSUMERS);
    let mut value = manifest("provider", vec![]);
    value["version"] = json!("1.1.0");
    let updated = input_value(&world, value, 2);
    install::install_inert_plugin(
        &world.trust,
        world.store.as_mut().unwrap(),
        updated.borrow(),
        &compatibility,
        || now,
    )
    .unwrap();
    let consumer = input(&world, "consumer", vec![service("middle")], true, 1);
    assert_eq!(
        install(&mut world, &consumer).err(),
        Some(ToolInstallError::Storage(InstallError::Dependency(
            crate::dependencies::DependencyError::VersionConflict
        )))
    );
    assert_eq!(world.metadata(CONSUMERS), before);
    assert!(world.metadata(CONSUMER_PENDING).is_none());
    assert!(!artifact(&world, &consumer).exists());
}
#[test]
fn every_other_pending_transaction_refuses_consumer_install_inspect_and_abort_without_parsing_it() {
    for key in [
        "package-install-pending-v1",
        "package-reconciliation-pending-v1",
        PENDING,
        "tool-archive-reconciliation-pending-v1",
        "tool-image-materialization-pending-v1",
        "tool-image-reconciliation-pending-v1",
    ] {
        let mut world = world();
        world.install().unwrap();
        let input = input(&world, "consumer", vec![], true, 1);
        set(&world, key, "CORRUPT");
        let expected = Some(ToolInstallError::Storage(InstallError::RecoveryPending));
        assert_eq!(install(&mut world, &input).err(), expected, "{key}");
        assert_eq!(inspect(&mut world).err(), expected);
        assert_eq!(abort(&mut world).err(), expected);
        assert_eq!(world.metadata(key).as_deref(), Some("CORRUPT"));
        assert!(world.metadata(CONSUMER_PENDING).is_none());
        assert!(!world.path.join("tool-bound-plugin-store-v1").exists());
    }
}
#[test]
fn current_signed_tool_revocation_after_publication_refuses_commit_but_allows_byte_preserving_abort(
) {
    let mut world = world();
    world.install().unwrap();
    let input = input(&world, "consumer", vec![], true, 1);
    let snapshot = world.trust.snapshot(world.store.as_mut().unwrap()).unwrap();
    let mut root: RootPayload = serde_json::from_slice(
        &decode_envelope(
            &serde_json::to_vec(&world.fixture.base.golden["rootEnvelope"]).unwrap(),
            ROOT_PAYLOAD_TYPE,
        )
        .unwrap()
        .payload,
    )
    .unwrap();
    root.version += 1;
    root.previous_root_sha256 = Some(snapshot.root_digest);
    root.revoked_package_digests
        .push(world.fixture.base.package.archive.sha256.clone());
    let envelope = signed(ROOT_PAYLOAD_TYPE, &root, &[1, 2]);
    let compatibility = compatibility(&world);
    let now = world.fixture.now();
    let trust = &world.trust;
    let result = consumers::install_fixture_with_hook(
        trust,
        world.store.as_mut().unwrap(),
        input.borrow(),
        &compatibility,
        || now,
        |phase, store| {
            if phase == Phase::BeforeCommit {
                trust.rotate(store, &envelope, now).unwrap();
            }
            Ok(())
        },
    );
    assert_eq!(
        result.err(),
        Some(ToolInstallError::Admission(ToolAdmissionError::Package(
            PackageError::PackageRevoked
        )))
    );
    assert!(world.metadata(CONSUMERS).is_none());
    assert!(world.metadata(CONSUMER_PENDING).is_some());
    assert!(artifact(&world, &input).exists());
    assert!(abort(&mut world).unwrap());
    assert!(inspect(&mut world).unwrap().is_empty());
    assert!(world.blob().exists());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn fresh_final_t0_clock_refuses_an_expired_root_and_preserves_the_pending_record() {
    let mut world = world();
    world.install().unwrap();
    let input = input(&world, "consumer", vec![], true, 1);
    let now = Cell::new(world.fixture.now());
    let expired = world.fixture.base.root.payload().expires_at + 1;
    let compatibility = compatibility(&world);
    assert_eq!(
        consumers::install_fixture_with_hook(
            &world.trust,
            world.store.as_mut().unwrap(),
            input.borrow(),
            &compatibility,
            || now.get(),
            |phase, _| {
                if phase == Phase::BeforeCommit {
                    now.set(expired);
                }
                Ok(())
            }
        )
        .err(),
        Some(ToolInstallError::Storage(InstallError::Package(
            PackageError::TrustExpired
        )))
    );
    assert!(world.metadata(CONSUMERS).is_none());
    assert!(world.metadata(CONSUMER_PENDING).is_some());
    assert!(artifact(&world, &input).exists());
}
#[test]
fn final_sql_substitution_rolls_back_consumer_inventory_and_retains_the_original_pending() {
    for key in [
        CONSUMERS,
        KEY,
        "package-trust-state-v1",
        CONSUMER_PENDING,
        "tool-image-inventory-v1",
        "package-install-pending-v1",
    ] {
        let mut world = world();
        world.install().unwrap();
        let input = input(&world, "consumer", vec![], true, 1);
        let original = world.metadata(key);
        let mut pending = None;
        assert!(with_hook(&mut world, &input, |phase, store| {
            if phase == Phase::BeforeCommit {
                pending = Some(store.connection.query_row("SELECT value FROM core_metadata WHERE key=?1", [CONSUMER_PENDING], |r| r.get::<_,String>(0)).unwrap());
                store.connection.execute_batch(&format!("CREATE TRIGGER substitute AFTER INSERT ON core_metadata WHEN NEW.key='{CONSUMERS}' BEGIN INSERT INTO core_metadata(key,value) VALUES('{key}','CORRUPT') ON CONFLICT(key) DO UPDATE SET value=excluded.value; END;")).unwrap();
            } Ok(())
        }).is_err(), "{key}");
        assert_eq!(world.metadata(CONSUMER_PENDING), pending);
        assert!(world.metadata(CONSUMERS).is_none());
        if !["package-trust-state-v1", CONSUMER_PENDING].contains(&key) {
            assert_eq!(world.metadata(key), original);
        }
        assert_ne!(
            world.metadata("package-trust-state-v1").as_deref(),
            Some("CORRUPT")
        );
        world
            .store
            .as_ref()
            .unwrap()
            .connection
            .execute_batch("DROP TRIGGER substitute")
            .unwrap();
        assert!(abort(&mut world).unwrap());
        assert!(inspect(&mut world).unwrap().is_empty());
    }
}
#[test]
fn abort_cannot_clear_its_journal_when_sql_fails_after_quarantine_and_can_retry_exact_bytes() {
    let mut world = world();
    world.install().unwrap();
    let input = input(&world, "consumer", vec![], true, 1);
    assert!(with_hook(&mut world, &input, |phase, _| {
        if phase == Phase::BeforeCommit {
            Err(InstallError::Storage.into())
        } else {
            Ok(())
        }
    })
    .is_err());
    let pending = world.metadata(CONSUMER_PENDING).unwrap();
    world.store.as_ref().unwrap().connection.execute_batch(&format!("CREATE TRIGGER deny_clear BEFORE DELETE ON core_metadata WHEN OLD.key='{CONSUMER_PENDING}' BEGIN SELECT RAISE(ABORT,'fixture'); END;")).unwrap();
    assert!(abort(&mut world).is_err());
    assert_eq!(
        world.metadata(CONSUMER_PENDING).as_deref(),
        Some(pending.as_str())
    );
    assert!(!artifact(&world, &input).exists());
    let group = fs::read_dir(world.path.join("tool-bound-plugin-store-v1/quarantine"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert_eq!(fs::read(group.join("package.zip")).unwrap(), input.archive);
    world
        .store
        .as_ref()
        .unwrap()
        .connection
        .execute_batch("DROP TRIGGER deny_clear")
        .unwrap();
    assert!(abort(&mut world).unwrap());
    assert_eq!(fs::read(group.join("package.zip")).unwrap(), input.archive);
}
#[test]
fn restored_pending_consumer_needs_controlled_trust_recovery_and_explicit_abort() {
    let mut world = world();
    authority(&mut world);
    world.install().unwrap();
    let input = input(&world, "consumer", vec![], true, 1);
    let mut backup = None;
    assert!(with_hook(&mut world, &input, |phase, store| {
        if phase == Phase::BeforeCommit {
            backup = Some(store.backup().unwrap());
            return Err(InstallError::Storage.into());
        }
        Ok(())
    })
    .is_err());
    let now = world.fixture.now();
    let checkpoint = world
        .trust
        .export_recovery_checkpoint(world.store.as_mut().unwrap(), now + 1)
        .unwrap();
    let pending = world.metadata(CONSUMER_PENDING);
    drop(world.store.take());
    DataStore::restore_backup(&world.path.join("runtime.sqlite"), &backup.unwrap()).unwrap();
    world.reopen();
    assert!(abort(&mut world).is_err());
    assert_eq!(world.metadata(CONSUMER_PENDING), pending);
    recover(&mut world, &checkpoint, now + 2);
    assert!(abort(&mut world).unwrap());
    assert!(inspect(&mut world).unwrap().is_empty());
    assert!(world.blob().exists());
    assert_no_effects(world.store.as_ref().unwrap());
}
#[test]
fn consumer_quarantine_shares_the_original_group_and_byte_budget_with_all_other_pools() {
    for byte_budget in [false, true] {
        let mut world = world();
        world.install().unwrap();
        let parent = world.path.join("tool-bound-plugin-store-v1/quarantine");
        fs::create_dir_all(&parent).unwrap();
        if byte_budget {
            for _ in 0..8 {
                let group = parent.join(format!("{}-stage", uuid::Uuid::new_v4()));
                fs::create_dir(&group).unwrap();
                fs::File::create(group.join("package.zip"))
                    .unwrap()
                    .set_len(33_554_432)
                    .unwrap();
            }
        } else {
            for _ in 0..383 {
                fs::create_dir(parent.join(format!("{}-stage", uuid::Uuid::new_v4()))).unwrap();
            }
        }
        let input = input(&world, "consumer", vec![], true, 1);
        assert_eq!(
            install(&mut world, &input).err(),
            Some(ToolInstallError::Storage(InstallError::BudgetExceeded))
        );
        assert!(world.metadata(CONSUMER_PENDING).is_none());
        assert!(world.metadata(CONSUMERS).is_none());
        assert!(!artifact(&world, &input).exists());
    }
}
#[test]
fn consumer_install_crash_child() {
    let Ok(path) = std::env::var("FLOWTOOLS_CONSUMER_CRASH_PROFILE") else {
        return;
    };
    let phase = std::env::var("FLOWTOOLS_CONSUMER_CRASH_PHASE").unwrap();
    let profile = Path::new(&path);
    assert_eq!(profile.parent().unwrap(), std::env::temp_dir());
    assert!(profile
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .strip_prefix("flowtools-tool-install-")
        .is_some_and(|s| uuid::Uuid::parse_str(s).is_ok_and(|id| id.to_string() == s)));
    assert!(profile.join("runtime.sqlite").is_file());
    let mut world = open(Path::new(&path), false, false);
    let input = input(&world, "consumer", vec![], true, 1);
    let _ = with_hook(&mut world, &input, |observed, _| {
        if format!("{observed:?}") == phase {
            std::process::exit(81);
        }
        Ok(())
    });
    panic!("consumer crash barrier was never reached");
}

#[test]
fn incompatible_consumer_inventory_profiles_and_duplicate_or_unknown_fields_preserve_original_bytes(
) {
    let mut world = world();
    world.install().unwrap();
    let input = input(&world, "consumer", vec![], true, 1);
    install(&mut world, &input).unwrap();
    let original = world.metadata(CONSUMERS).unwrap();
    for attack in [
        "version",
        "profile",
        "old-lock",
        "duplicate-json",
        "unknown",
        "oversize",
    ] {
        let mut value: Value = serde_json::from_str(&original).unwrap();
        match attack {
            "version" => value["formatVersion"] = json!(2),
            "profile" => value["profileDigest"] = json!("0".repeat(64)),
            "old-lock" => {
                value["records"][0]["lock"]["profile"] = json!("signed-plugin-archives-v1")
            }
            "unknown" => value["approved"] = json!(true),
            _ => (),
        }
        let text = match attack {
            "duplicate-json" => format!("{{\"formatVersion\":1,{}", &original[1..]),
            "oversize" => " ".repeat(16_777_217),
            _ => value.to_string(),
        };
        set(&world, CONSUMERS, &text);
        assert!(inspect(&mut world).is_err(), "{attack}");
        assert!(abort(&mut world).is_err(), "{attack}");
        assert!(world.pins().is_err(), "{attack}");
        assert_eq!(world.metadata(CONSUMERS).as_deref(), Some(text.as_str()));
        assert!(artifact(&world, &input).exists());
        assert!(world.blob().exists());
    }
    set(&world, CONSUMERS, &original);
    assert_eq!(inspect(&mut world).unwrap().len(), 1);
}
#[test]
fn malformed_or_forged_consumer_pending_never_selects_another_path_or_clears_itself() {
    let mut world = world();
    world.install().unwrap();
    let input = input(&world, "consumer", vec![], true, 1);
    assert!(with_hook(&mut world, &input, |phase, _| {
        if phase == Phase::BeforeCommit {
            Err(InstallError::Storage.into())
        } else {
            Ok(())
        }
    })
    .is_err());
    let original = world.metadata(CONSUMER_PENDING).unwrap();
    for attack in [
        "version",
        "id",
        "artifact",
        "before",
        "old-lock",
        "existing",
        "duplicate-json",
        "unknown",
    ] {
        let mut value: Value = serde_json::from_str(&original).unwrap();
        match attack {
            "version" => value["formatVersion"] = json!(2),
            "id" => value["id"] = json!("../outside"),
            "artifact" => value["record"]["artifactId"] = json!("../outside"),
            "before" => value["beforeDigest"] = json!("0".repeat(64)),
            "old-lock" => value["record"]["lock"]["profile"] = json!("signed-plugin-archives-v1"),
            "existing" => value["newArtifact"] = json!(false),
            "unknown" => value["approved"] = json!(true),
            _ => (),
        }
        let text = if attack == "duplicate-json" {
            format!("{{\"formatVersion\":1,{}", &original[1..])
        } else {
            value.to_string()
        };
        set(&world, CONSUMER_PENDING, &text);
        assert!(abort(&mut world).is_err(), "{attack}");
        assert_eq!(
            world.metadata(CONSUMER_PENDING).as_deref(),
            Some(text.as_str())
        );
        assert!(artifact(&world, &input).exists());
        assert_eq!(
            fs::read_dir(world.path.join("tool-bound-plugin-store-v1/quarantine"))
                .unwrap()
                .count(),
            0
        );
    }
    set(&world, CONSUMER_PENDING, &original);
    assert!(abort(&mut world).unwrap());
}
#[test]
fn actual_process_exit_at_all_five_barriers_retains_or_commits_exact_installed_tool_bindings() {
    for phase in [
        Phase::JournalCommitted,
        Phase::StagePrepared,
        Phase::ArtifactPublished,
        Phase::BeforeCommit,
        Phase::Committed,
    ] {
        let mut world = world();
        world.install().unwrap();
        let input = input(&world, "consumer", vec![], true, 1);
        let raw = world.metadata(KEY);
        drop(world.store.take());
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "packages::tests::tool_tests::install_tests::consumer_tests::recovery_tests::consumer_install_crash_child", "--nocapture", "--test-threads=1"])
            .env("FLOWTOOLS_CONSUMER_CRASH_PROFILE", &world.path)
            .env("FLOWTOOLS_CONSUMER_CRASH_PHASE", format!("{phase:?}"))
            .stdin(std::process::Stdio::null()).spawn().unwrap();
        let deadline = Instant::now() + Duration::from_secs(20);
        let status = loop {
            if let Some(status) = child.try_wait().unwrap() {
                break status;
            }
            if Instant::now() >= deadline {
                child.kill().unwrap();
                child.wait().unwrap();
                panic!("owned consumer crash fixture exceeded 20s: {phase:?}");
            }
            std::thread::sleep(Duration::from_millis(10));
        };
        assert_eq!(status.code(), Some(81), "{phase:?}");
        world.reopen();
        if phase == Phase::Committed {
            let records = inspect(&mut world).unwrap();
            assert_eq!(records.len(), 1);
            assert_eq!(
                records[0].dependency_lock().tools[0].package_digest,
                world.fixture.base.package.archive.sha256
            );
            assert!(!abort(&mut world).unwrap());
            assert_eq!(
                fs::read(artifact(&world, &input).join("package.zip")).unwrap(),
                input.archive
            );
        } else {
            assert!(world.metadata(CONSUMER_PENDING).is_some());
            assert!(abort(&mut world).unwrap());
            assert!(inspect(&mut world).unwrap().is_empty());
            if phase != Phase::JournalCommitted {
                let group = fs::read_dir(world.path.join("tool-bound-plugin-store-v1/quarantine"))
                    .unwrap()
                    .next()
                    .unwrap()
                    .unwrap()
                    .path();
                assert_eq!(fs::read(group.join("package.zip")).unwrap(), input.archive);
            }
        }
        assert_eq!(world.metadata(KEY), raw);
        assert_eq!(
            fs::read(world.blob().join("package.zip")).unwrap(),
            world.fixture.base.archive
        );
        assert_no_effects(world.store.as_ref().unwrap());
    }
}
