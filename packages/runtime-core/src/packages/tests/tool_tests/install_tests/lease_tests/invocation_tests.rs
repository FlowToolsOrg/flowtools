//! Current T0 effect admission with real held canaries, never executable tools.
use super::*;
use crate::{
    broker::Scope,
    packages::{
        tools::{install::leases, media::MediaActionV1, ToolProgram},
        RootPayload,
    },
    services::ServiceTarget,
};
use std::{cell::Cell, sync::atomic::Ordering};
const ACTIONS: &[&str] = &["inspect-v1", "extract-audio-wav-v1", "transcode-webm-v1"];

fn declared(command: &mut Value, actions: &[&str]) {
    command["effects"] = json!(["tool-execute"]);
    command["permissions"] = json!([{"capability":"tool","operations":["execute"],"scopes":actions.iter().map(|action|format!("lock:ffmpeg:{action}")).collect::<Vec<_>>()}]);
}
fn tool_catalog(world: &World, service: bool, actions: &[&str]) -> BuiltinCatalog {
    let ids: &[&str] = if service {
        &["consumer", "middle"]
    } else {
        &["consumer"]
    };
    let mut manifests = catalog(world, ids)
        .list()
        .map(|(_, value)| value.clone())
        .collect::<Vec<_>>();
    for manifest in &mut manifests {
        declared(&mut manifest["commands"][0], actions);
        declared(&mut manifest["services"][0]["operations"][0], actions);
    }
    if service {
        manifests
            .iter_mut()
            .find(|manifest| manifest["id"] == "consumer")
            .unwrap()["dependencies"]["services"] = json!([{
            "publisher":"fixture-publisher","id":"middle","version":"^1.0.0","service":"text-transform","interfaceVersion":"^1.0.0"
        }]);
    }
    BuiltinCatalog::from_host_manifests(manifests).unwrap()
}
fn grant_tool(
    core: &mut RuntimeCore,
    catalog: &BuiltinCatalog,
    id: &str,
    command: &str,
    actions: &[&str],
    calls: u32,
) {
    let connection = format!("admin-tool-{}", uuid::Uuid::new_v4());
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
                    effects: vec!["tool-execute".into()],
                    scopes: actions
                        .iter()
                        .map(|action| Scope::ToolLock {
                            lock: "ffmpeg".into(),
                            action: (*action).into(),
                        })
                        .collect(),
                    expires_at: now() + 60_000.0,
                    max_calls: calls,
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
fn revoke_tool(core: &mut RuntimeCore, id: &str, command: &str) {
    let connection = format!("admin-revoke-{}", uuid::Uuid::new_v4());
    let admin = open_on(core, &connection, "admin");
    let outcome = core
        .handle(
            &connection,
            request(
                Call::Revoke(PermissionKey {
                    plugin_id: id.into(),
                    command_id: command.into(),
                    target: GrantTarget::Cli,
                }),
                Some(admin),
            ),
        )
        .outcome;
    assert!(matches!(outcome, Outcome::Permissions(_)), "{outcome:?}");
    core.disconnect(&connection);
}
fn service(core: &mut RuntimeCore, root: &str) -> String {
    core.prepare_service(
        root,
        ServiceTarget {
            publisher: "fixture-publisher".into(),
            id: "middle".into(),
            service: "text-transform".into(),
            operation: "convert".into(),
        },
        json!({"text":"fixture"}),
    )
    .unwrap()
    .spec
    .run_id
}
#[test]
fn running_root_gets_exact_accepted_pins_and_finite_roles_without_replacing_its_lease() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = tool_catalog(&world, false, ACTIONS);
    let mut core = core(&mut world, catalog.clone(), false);
    grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 8);
    let receipt = submit(&mut core, operation("consumer", "tool-use"));
    assert_eq!(
        core.invoke_media_tool(&receipt.run_id, MediaActionV1::Inspect, |_| panic!(
            "Queued root"
        )),
        Err::<(), _>(ErrorCode::SessionInvalid)
    );
    assert!(core.take_run(&receipt.run_id).is_some());
    let original = row(&mut core, &receipt.run_id)["toolArtifacts"].clone();
    let calls = Cell::new(0);
    for (action, program) in [
        (MediaActionV1::Inspect, ToolProgram::Ffprobe),
        (MediaActionV1::ExtractAudioWav, ToolProgram::Ffmpeg),
        (MediaActionV1::TranscodeWebm, ToolProgram::Ffmpeg),
    ] {
        core.invoke_media_tool(&receipt.run_id, action, |tool| {
            assert_eq!(tool.action(), action);
            assert_eq!(tool.program(), program);
            assert_eq!(tool.consumer().id, "consumer");
            assert_eq!(tool.tool().consumer, tool.consumer().identity());
            assert_eq!(
                tool.tool().digest,
                world.fixture.base.package.archive.sha256
            );
            calls.set(calls.get() + 1);
            Ok(())
        })
        .unwrap();
    }
    assert_eq!(calls.get(), 3);
    assert_eq!(row(&mut core, &receipt.run_id)["toolArtifacts"], original);
    core.finish(&receipt.run_id, Ok(json!("fixture")));
    assert_eq!(
        core.invoke_media_tool(&receipt.run_id, MediaActionV1::Inspect, |_| panic!(
            "Terminal root"
        )),
        Err::<(), _>(ErrorCode::SessionInvalid)
    );
}
#[test]
fn manifest_and_grant_scope_refusals_precede_any_new_trust_observation() {
    for case in ["undeclared", "manifest", "grant"] {
        let mut world = World::new();
        prepare_image(&mut world);
        let catalog = if case == "undeclared" {
            catalog(&world, &["consumer"])
        } else {
            tool_catalog(
                &world,
                false,
                if case == "manifest" {
                    &ACTIONS[..1]
                } else {
                    ACTIONS
                },
            )
        };
        let (mut core, clock) = core_with_clock(&mut world, catalog.clone(), false);
        if case == "undeclared" {
            grant(&mut core, &catalog, "consumer");
        } else {
            grant_tool(
                &mut core,
                &catalog,
                "consumer",
                "convert",
                if case == "grant" {
                    &ACTIONS[..1]
                } else {
                    ACTIONS
                },
                8,
            );
        }
        let receipt = submit(&mut core, operation("consumer", case));
        assert!(core.take_run(&receipt.run_id).is_some());
        clock.store(world.fixture.now() + 1, Ordering::SeqCst);
        let denied =
            core.invoke_media_tool(&receipt.run_id, MediaActionV1::ExtractAudioWav, |_| {
                panic!("Scope refused before callback")
            });
        assert_eq!(
            denied,
            Err::<(), _>(if case == "undeclared" {
                ErrorCode::CapabilityUndeclared
            } else {
                ErrorCode::ScopeDenied
            }),
            "{case}"
        );
        core.with_tool_fixture_store(|store| {
            assert_eq!(
                world.trust.snapshot(store).unwrap().observed_time,
                world.fixture.now(),
                "{case}"
            )
        });
        core.finish(&receipt.run_id, Err(ErrorCode::Aborted));
    }
}
#[test]
fn tool_calls_charge_the_ordinary_budget_once_and_exhaustion_does_not_readmit_artifacts() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = tool_catalog(&world, false, ACTIONS);
    let (mut core, clock) = core_with_clock(&mut world, catalog.clone(), false);
    grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 1);
    let receipt = submit(&mut core, operation("consumer", "budget"));
    assert!(core.take_run(&receipt.run_id).is_some());
    core.invoke_media_tool(&receipt.run_id, MediaActionV1::Inspect, |_| Ok(()))
        .unwrap();
    clock.store(world.fixture.now() + 1, Ordering::SeqCst);
    assert_eq!(
        core.invoke_media_tool(&receipt.run_id, MediaActionV1::Inspect, |_| panic!(
            "Exhausted budget"
        )),
        Err::<(), _>(ErrorCode::BudgetExceeded)
    );
    core.with_tool_fixture_store(|store| {
        assert_eq!(
            world.trust.snapshot(store).unwrap().observed_time,
            world.fixture.now()
        )
    });
    core.finish(&receipt.run_id, Ok(json!("fixture")));
}
#[test]
fn revoke_or_cancel_after_artifact_io_refuses_the_effect_without_dropping_handles() {
    for change in ["revoke", "regrant", "cancel"] {
        let mut world = World::new();
        let image = prepare_image(&mut world);
        let catalog = tool_catalog(&world, false, ACTIONS);
        let mut core = core(&mut world, catalog.clone(), false);
        grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 8);
        let receipt = submit(&mut core, operation("consumer", "interleaved"));
        assert!(core.take_run(&receipt.run_id).is_some());
        let result = core.invoke_media_fixture_with_hook(
            &receipt.run_id,
            MediaActionV1::Inspect,
            |_| panic!("Stale permission"),
            |core| {
                match change {
                    "cancel" => core.cancel(&receipt.run_id),
                    "regrant" => grant_tool(core, &catalog, "consumer", "convert", ACTIONS, 8),
                    _ => revoke_tool(core, "consumer", "convert"),
                }
                Ok(())
            },
        );
        assert_eq!(
            result,
            Err::<(), _>(if change == "cancel" {
                ErrorCode::Aborted
            } else {
                ErrorCode::GrantRevoked
            })
        );
        assert_eq!(
            row(&mut core, &receipt.run_id)["toolArtifacts"]["retained"],
            true
        );
        let path = image.join("bin/ffmpeg.exe");
        writable(&path);
        assert!(write_handle(&path).is_err());
        readonly(&path);
        core.finish(&receipt.run_id, Err(ErrorCode::Aborted));
    }
}
#[test]
fn unknown_job_changes_after_artifact_io_stop_the_host_before_the_callback() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = tool_catalog(&world, false, ACTIONS);
    let mut core = core(&mut world, catalog.clone(), false);
    grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 8);
    let receipt = submit(&mut core, operation("consumer", "unknown-row"));
    assert!(core.take_run(&receipt.run_id).is_some());
    let mut unknown = row(&mut core, &receipt.run_id);
    unknown["idempotencyKey"] = json!("changed-before-effect");
    let result = core.invoke_media_fixture_with_hook(
        &receipt.run_id,
        MediaActionV1::Inspect,
        |_| panic!("Unknown task state"),
        |core| {
            core.with_tool_fixture_store(|store| {
                store
                    .connection
                    .execute(
                        "UPDATE jobs SET metadata=?1 WHERE run_id=?2",
                        rusqlite::params![serde_json::to_string(&unknown).unwrap(), receipt.run_id],
                    )
                    .unwrap()
            });
            Ok(())
        },
    );
    assert_eq!(result, Err::<(), _>(ErrorCode::StoreCorrupt));
    assert!(core.stopping());
    assert_eq!(row(&mut core, &receipt.run_id), unknown);
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
fn current_clock_and_signed_revocation_refuse_effects_after_a_run_has_started() {
    for reason in ["expired", "rollback", "revoked"] {
        let mut world = World::new();
        let image = prepare_image(&mut world);
        let catalog = tool_catalog(&world, false, ACTIONS);
        let (mut core, clock) = core_with_clock(&mut world, catalog.clone(), false);
        grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 8);
        let receipt = submit(&mut core, operation("consumer", reason));
        assert!(core.take_run(&receipt.run_id).is_some());
        match reason {
            "expired" => clock.store(world.fixture.base.package.expires_at, Ordering::SeqCst),
            "rollback" => clock.store(world.fixture.now() - 1, Ordering::SeqCst),
            _ => {
                let envelope =
                    serde_json::to_vec(&world.fixture.base.golden["rootEnvelope"]).unwrap();
                let mut root: RootPayload = serde_json::from_slice(
                    &decode_envelope(&envelope, ROOT_PAYLOAD_TYPE)
                        .unwrap()
                        .payload,
                )
                .unwrap();
                root.version += 1;
                root.previous_root_sha256 = Some(core.with_tool_fixture_store(|store| {
                    world.trust.snapshot(store).unwrap().root_digest
                }));
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
            }
        }
        assert!(
            core.invoke_media_tool::<()>(&receipt.run_id, MediaActionV1::Inspect, |_| panic!(
                "Current tool admission refused"
            ))
            .is_err(),
            "{reason}"
        );
        assert_eq!(
            row(&mut core, &receipt.run_id)["toolArtifacts"]["retained"],
            true
        );
        let path = image.join("bin/ffmpeg.exe");
        writable(&path);
        assert!(write_handle(&path).is_err());
        readonly(&path);
    }
}
#[test]
fn every_pending_transaction_blocks_tool_use_without_clearing_the_journal() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = tool_catalog(&world, false, ACTIONS);
    let mut core = core(&mut world, catalog.clone(), false);
    grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 8);
    let receipt = submit(&mut core, operation("consumer", "pending"));
    assert!(core.take_run(&receipt.run_id).is_some());
    for key in [
        "package-install-pending-v1",
        "package-reconciliation-pending-v1",
        "tool-archive-install-pending-v1",
        "tool-archive-reconciliation-pending-v1",
        "tool-image-materialization-pending-v1",
        "tool-image-reconciliation-pending-v1",
        "tool-consumer-install-pending-v1",
        "tool-consumer-reconciliation-pending-v1",
    ] {
        core.with_tool_fixture_store(|store| {
            store
                .connection
                .execute(
                    "INSERT INTO core_metadata(key,value) VALUES(?1,'owned-inert-fixture')",
                    [key],
                )
                .unwrap()
        });
        assert_eq!(
            core.invoke_media_tool(&receipt.run_id, MediaActionV1::Inspect, |_| panic!(
                "Pending transaction"
            )),
            Err::<(), _>(ErrorCode::RecoveryPending),
            "{key}"
        );
        core.with_tool_fixture_store(|store| {
            assert_eq!(
                store
                    .connection
                    .query_row(
                        "SELECT value FROM core_metadata WHERE key=?1",
                        [key],
                        |row| row.get::<_, String>(0)
                    )
                    .unwrap(),
                "owned-inert-fixture"
            );
            store
                .connection
                .execute(
                    "DELETE FROM core_metadata WHERE key=?1 AND value='owned-inert-fixture'",
                    [key],
                )
                .unwrap();
        });
    }
    core.invoke_media_tool(&receipt.run_id, MediaActionV1::Inspect, |_| Ok(()))
        .unwrap();
    core.finish(&receipt.run_id, Ok(json!("fixture")));
}
#[test]
fn service_use_selects_its_own_consumer_pin_and_refuses_a_queued_or_missing_binding() {
    for missing in [false, true] {
        let mut world = World::new();
        prepare_image(&mut world);
        let catalog = tool_catalog(&world, true, ACTIONS);
        let catalog = if missing {
            let mut manifests = catalog
                .list()
                .map(|(_, manifest)| manifest.clone())
                .collect::<Vec<_>>();
            manifests
                .iter_mut()
                .find(|manifest| manifest["id"] == "middle")
                .unwrap()["dependencies"]["tools"] = json!([]);
            BuiltinCatalog::from_host_manifests(manifests).unwrap()
        } else {
            catalog
        };
        let mut core = core(&mut world, catalog.clone(), false);
        grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 8);
        grant_tool(
            &mut core,
            &catalog,
            "middle",
            "svc:text-transform:convert",
            ACTIONS,
            8,
        );
        let receipt = submit(&mut core, operation("consumer", "service-pin"));
        assert!(core.take_run(&receipt.run_id).is_some());
        let call = service(&mut core, &receipt.run_id);
        assert_eq!(
            core.invoke_media_tool(&call, MediaActionV1::Inspect, |_| panic!("Queued service")),
            Err::<(), _>(ErrorCode::SessionInvalid)
        );
        core.start_service(&call).unwrap();
        let calls = Cell::new(0);
        let result = core.invoke_media_tool(&call, MediaActionV1::Inspect, |tool| {
            assert_eq!(tool.consumer().id, "middle");
            assert_eq!(tool.tool().consumer, tool.consumer().identity());
            calls.set(calls.get() + 1);
            Ok(())
        });
        if missing {
            assert_eq!(result, Err(ErrorCode::DependencyArtifactMismatch));
            assert_eq!(calls.get(), 0);
        } else {
            result.unwrap();
            assert_eq!(calls.get(), 1);
        }
        let _ = core.finish_service(&call, Err(ErrorCode::Aborted));
        core.finish(&receipt.run_id, Err(ErrorCode::Aborted));
    }
}
#[test]
fn delegated_tool_effects_require_both_parent_and_provider_scopes_at_the_effect() {
    for parent_denied in [false, true] {
        let mut world = World::new();
        prepare_image(&mut world);
        let catalog = tool_catalog(&world, true, ACTIONS);
        let mut core = core(&mut world, catalog.clone(), false);
        grant_tool(
            &mut core,
            &catalog,
            "consumer",
            "convert",
            if parent_denied {
                &ACTIONS[..1]
            } else {
                ACTIONS
            },
            8,
        );
        grant_tool(
            &mut core,
            &catalog,
            "middle",
            "svc:text-transform:convert",
            if parent_denied {
                ACTIONS
            } else {
                &ACTIONS[..1]
            },
            8,
        );
        let receipt = submit(&mut core, operation("consumer", "delegated-scope"));
        assert!(core.take_run(&receipt.run_id).is_some());
        let call = service(&mut core, &receipt.run_id);
        core.start_service(&call).unwrap();
        assert_eq!(
            core.invoke_media_tool(&call, MediaActionV1::TranscodeWebm, |_| panic!(
                "Missing delegation scope"
            )),
            Err::<(), _>(ErrorCode::ScopeDenied)
        );
        let _ = core.finish_service(&call, Err(ErrorCode::Aborted));
        core.finish(&receipt.run_id, Err(ErrorCode::Aborted));
    }
}
#[test]
fn provider_revocation_during_tool_io_cannot_fall_back_to_the_root_grant() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = tool_catalog(&world, true, ACTIONS);
    let mut core = core(&mut world, catalog.clone(), false);
    grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 8);
    grant_tool(
        &mut core,
        &catalog,
        "middle",
        "svc:text-transform:convert",
        ACTIONS,
        8,
    );
    let receipt = submit(&mut core, operation("consumer", "provider-revoke"));
    assert!(core.take_run(&receipt.run_id).is_some());
    let call = service(&mut core, &receipt.run_id);
    core.start_service(&call).unwrap();
    assert_eq!(
        core.invoke_media_fixture_with_hook(
            &call,
            MediaActionV1::Inspect,
            |_| panic!("Revoked provider"),
            |core| {
                revoke_tool(core, "middle", "svc:text-transform:convert");
                Ok(())
            }
        ),
        Err::<(), _>(ErrorCode::GrantRevoked)
    );
    let _ = core.finish_service(&call, Err(ErrorCode::Aborted));
    core.finish(&receipt.run_id, Err(ErrorCode::Aborted));
}
#[test]
fn a_running_service_cannot_hide_a_denied_tool_effect_by_returning_success() {
    for media_adapter in [true, false] {
        let mut world = World::new();
        prepare_image(&mut world);
        let catalog = tool_catalog(&world, true, ACTIONS);
        let mut core = core(&mut world, catalog.clone(), false);
        grant_tool(&mut core, &catalog, "consumer", "convert", &ACTIONS[..1], 8);
        grant_tool(
            &mut core,
            &catalog,
            "middle",
            "svc:text-transform:convert",
            ACTIONS,
            8,
        );
        let receipt = submit(&mut core, operation("consumer", "sticky-error"));
        assert!(core.take_run(&receipt.run_id).is_some());
        let call = service(&mut core, &receipt.run_id);
        core.start_service(&call).unwrap();
        let denied = if media_adapter {
            core.invoke_media_tool(&call, MediaActionV1::TranscodeWebm, |_| {
                panic!("Parent scope denied")
            })
        } else {
            core.invoke_service(
                &call,
                &leases::media_operation(MediaActionV1::TranscodeWebm),
                |_| panic!("Parent scope denied for ordinary service effect"),
            )
        };
        assert_eq!(denied, Err::<(), _>(ErrorCode::ScopeDenied));
        assert_eq!(
            core.finish_service(&call, Ok(json!("ignored rejection"))),
            Err(ErrorCode::ScopeDenied)
        );
        let diagnostic = core.service_diagnostics(&receipt.run_id).unwrap();
        assert_eq!(diagnostic[0].state, "failed");
        assert_eq!(diagnostic[0].failure_code, Some(ErrorCode::ScopeDenied));
        core.finish(&receipt.run_id, Err(ErrorCode::ExecutionFailed));
    }
}
#[test]
fn a_deadline_crossed_after_artifact_io_still_refuses_the_effect_and_keeps_roots() {
    let mut world = World::new();
    let image = prepare_image(&mut world);
    let catalog = tool_catalog(&world, false, ACTIONS);
    let mut core = core(&mut world, catalog.clone(), false);
    grant_tool(&mut core, &catalog, "consumer", "convert", ACTIONS, 8);
    let operation = operation("consumer", "deadline");
    let deadline = operation.deadline;
    let receipt = submit(&mut core, operation);
    assert!(core.take_run(&receipt.run_id).is_some());
    let result = core.invoke_media_fixture_with_hook(
        &receipt.run_id,
        MediaActionV1::Inspect,
        |_| panic!("Expired command deadline"),
        |_| {
            let remaining = (deadline - now()).clamp(0.0, 20_000.0).ceil() as u64;
            std::thread::sleep(std::time::Duration::from_millis(remaining + 5));
            Ok(())
        },
    );
    assert_eq!(result, Err::<(), _>(ErrorCode::Timeout));
    assert_eq!(
        row(&mut core, &receipt.run_id)["toolArtifacts"]["retained"],
        true
    );
    let path = image.join("bin/ffmpeg.exe");
    writable(&path);
    assert!(write_handle(&path).is_err());
    readonly(&path);
}
#[test]
fn a_management_only_reopen_never_issues_a_tool_effect_context() {
    let mut world = World::new();
    prepare_image(&mut world);
    let catalog = tool_catalog(&world, false, ACTIONS);
    let mut first = core(&mut world, catalog.clone(), false);
    grant_tool(&mut first, &catalog, "consumer", "convert", ACTIONS, 8);
    let mut operation = operation("consumer", "inert-manager");
    operation.background = true;
    let receipt = submit(&mut first, operation);
    let original = row(&mut first, &receipt.run_id)["toolArtifacts"].clone();
    drop(first);
    world.reopen();
    let mut manager = core(&mut world, catalog, true);
    assert!(manager.pending_runs().is_empty());
    assert_eq!(
        manager.invoke_media_tool(&receipt.run_id, MediaActionV1::Inspect, |_| panic!(
            "Management-only Host"
        )),
        Err::<(), _>(ErrorCode::SessionInvalid)
    );
    assert_eq!(
        row(&mut manager, &receipt.run_id)["toolArtifacts"],
        original
    );
}
