use super::*;

fn profile() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "flowtools-validation-g4-recovery-{}",
        Uuid::new_v4()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}
fn load(path: &std::path::Path) -> Result<RuntimeCore, ErrorCode> {
    RuntimeCore::managed(
        HashMap::from([("cli".into(), "local-cli".into())]),
        DataStore::open(&path.join("runtime.sqlite"))?,
        false,
        false,
    )
}

#[test]
fn migration_rollback_reopens_old_catalog_and_unload_survives_restart() {
    let path = profile();
    let mut core = load(&path).unwrap();
    let plan = core.provider_unload_plan("plugin-base64-encoder").unwrap();
    core.begin_provider_change(plan.clone(), &plan.digest, false)
        .unwrap();
    assert_eq!(
        core.commit_provider_change(&plan.digest, |tx| {
            tx.execute(
                "INSERT INTO core_metadata(key,value) VALUES('migration-private','data')",
                [],
            )
            .unwrap();
            Err(ErrorCode::StorageFailed)
        }),
        Err(ErrorCode::StorageFailed)
    );
    assert!(!core.stopping);
    assert_eq!(core.data.connection.query_row("SELECT count(*) FROM core_metadata WHERE key IN ('migration-private','service-catalog-state-v1')",[],|r|r.get::<_,i64>(0)).unwrap(),0);
    assert!(std::fs::read_dir(&path).unwrap().any(|entry| entry
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("runtime-backup-")));
    let plan = core.provider_unload_plan("plugin-base64-encoder").unwrap();
    core.begin_provider_change(plan.clone(), &plan.digest, false)
        .unwrap();
    core.commit_provider_change(&plan.digest, |_| Ok(()))
        .unwrap();
    drop(core);
    let restarted = load(&path).unwrap();
    assert!(!restarted
        .services
        .enabled(&crate::dependencies::DependencyIdentity {
            publisher: "flowtools".into(),
            id: "plugin-base64-encoder".into()
        }));
}

#[test]
fn failed_commit_stops_admission_and_prepared_journal_requires_recovery() {
    let path = profile();
    let mut core = load(&path).unwrap();
    core.data.connection.execute_batch("PRAGMA foreign_keys=ON; CREATE TABLE parent(id INTEGER PRIMARY KEY); CREATE TABLE child(id INTEGER REFERENCES parent(id) DEFERRABLE INITIALLY DEFERRED);").unwrap();
    let plan = core.provider_unload_plan("plugin-base64-encoder").unwrap();
    core.begin_provider_change(plan.clone(), &plan.digest, false)
        .unwrap();
    assert_eq!(
        core.commit_provider_change(&plan.digest, |tx| {
            tx.execute("INSERT INTO child(id) VALUES(999)", []).unwrap();
            Ok(())
        }),
        Err(ErrorCode::StorageFailed)
    );
    assert!(core.stopping);
    drop(core);
    assert_eq!(load(&path).err(), Some(ErrorCode::RecoveryPending));
}

#[test]
fn interrupted_service_metadata_restores_without_sessions_and_rejects_forged_consumer() {
    let path = profile();
    let mut manifests: BTreeMap<_, _> = BuiltinCatalog::embedded()
        .list()
        .map(|(id, m)| (id.clone(), m.clone()))
        .collect();
    let provider = manifests.get_mut("plugin-text-ops").unwrap();
    provider["entries"]["services"] = provider["entries"]["executor"].clone();
    provider["services"] =
        json!([{"id":"counter","version":"1.0.0","operations":provider["commands"]}]);
    manifests.get_mut("plugin-base64-encoder").unwrap()["dependencies"]["services"] = json!([{"publisher":"flowtools","id":"plugin-text-ops","version":"^0.1.0","service":"counter","interfaceVersion":"^1.0.0"}]);
    let catalog = BuiltinCatalog::from_host_manifests(manifests.into_values().collect()).unwrap();
    let tokens = || {
        HashMap::from([
            ("cli".into(), "local-cli".into()),
            ("manager".into(), "local-manager".into()),
        ])
    };
    let restore = || {
        RuntimeCore::managed_catalog(
            tokens(),
            DataStore::open(&path.join("runtime.sqlite")).unwrap(),
            false,
            false,
            catalog.clone(),
        )
    };
    let mut core = restore().unwrap();
    let request = |call, session| Request {
        version: PROTOCOL_MAJOR,
        request_id: Uuid::new_v4().to_string(),
        session,
        call,
    };
    let open = |core: &mut RuntimeCore, connection: &str| {
        let response = core.handle(
            connection,
            request(
                Call::Open(OpenSession {
                    token: connection.into(),
                    client_version: CLIENT_VERSION.into(),
                    expected_instance_id: None,
                }),
                None,
            ),
        );
        let Outcome::Session(session) = response.outcome else {
            panic!("Session")
        };
        session
    };
    let cli = open(&mut core, "cli");
    let manager = open(&mut core, "manager");
    for (id, command_id) in [
        ("plugin-base64-encoder", "run"),
        ("plugin-text-ops", "svc:counter:run"),
    ] {
        let manifest = catalog
            .list()
            .find(|(key, _)| key.as_str() == id)
            .unwrap()
            .1;
        let response = core.handle(
            "manager",
            request(
                Call::Grant(PermissionGrant {
                    plugin_id: id.into(),
                    command_id: command_id.into(),
                    target: GrantTarget::Cli,
                    package_digest: digest(manifest),
                    effects: vec![],
                    scopes: vec![],
                    expires_at: now() + 60000.0,
                    max_calls: 8,
                    cold_start: false,
                    background: false,
                }),
                Some(manager.clone()),
            ),
        );
        assert!(matches!(response.outcome, Outcome::Permissions(_)));
    }
    let response = core.handle(
        "cli",
        request(
            Call::Submit(SubmitJob {
                plugin_id: "plugin-base64-encoder".into(),
                command_id: "run".into(),
                input: json!({"text":"private-input"}),
                idempotency_key: "recovery-fixture".into(),
                background: false,
                deadline: now() + 10000.0,
            }),
            Some(cli),
        ),
    );
    let Outcome::Receipt(receipt) = response.outcome else {
        panic!("Receipt")
    };
    core.take_run(&receipt.run_id).unwrap();
    let call = core
        .prepare_service(
            &receipt.run_id,
            ServiceTarget {
                publisher: "flowtools".into(),
                id: "plugin-text-ops".into(),
                service: "counter".into(),
                operation: "run".into(),
            },
            json!({"setA":"private-input","setB":"other-private"}),
        )
        .unwrap();
    core.start_service(&call.spec.run_id).unwrap();
    // Simulate loss of the Host after persisted admission, without fabricated execution.
    drop(core);
    let restarted = restore().unwrap();
    assert_eq!(restarted.active_service_calls(), 0);
    let diagnostic = restarted.service_diagnostics(&receipt.run_id).unwrap();
    assert_eq!(diagnostic[0].state, "interrupted");
    assert_eq!(
        diagnostic[0].failure_code,
        Some(ErrorCode::ExecutionInterrupted)
    );
    let mut record = restarted.data.load_jobs().unwrap().remove(0);
    record["serviceCalls"][0]["consumer"]["id"] = json!("plugin-random-picker");
    restarted.data.save_job(&receipt.run_id, &record).unwrap();
    drop(restarted);
    assert_eq!(restore().err(), Some(ErrorCode::StoreCorrupt));
}
