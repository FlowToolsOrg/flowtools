use super::*;
use flowtools_runtime_core::{
    broker::Scope,
    catalog::{digest, BuiltinCatalog},
    services::ProviderChangePlan,
};
use std::collections::{BTreeMap, HashMap};

#[tokio::test]
async fn concurrent_activation_serializes_providers_and_cancel_preserves_other_root() {
    let fixture = Fixture::new().await;
    let (first, task1) = fixture.run("wait").await;
    // B already owns its provider gate; overlap B/C startup with the second A.
    fixture.wait_running_provider(&first, "fixture-b").await;
    let (second, task2) = fixture.run("echo").await;
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let core = fixture.core.lock().await;
            core.check_run(&first)
                .expect("First root must remain valid until both service states are ready");
            core.check_run(&second)
                .expect("Second root must remain valid until both service states are ready");
            if core
                .service_diagnostics(&second)
                .unwrap()
                .iter()
                .any(|call| call.provider.id == "fixture-b" && call.state == "queued")
                && core
                    .service_diagnostics(&first)
                    .unwrap()
                    .iter()
                    .any(|call| call.provider.id == "fixture-c" && call.state == "running")
            {
                assert_eq!(
                    core.service_diagnostics(&first)
                        .unwrap()
                        .iter()
                        .filter(|call| call.state == "running")
                        .count(),
                    2
                );
                break;
            }
            drop(core);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    Fixture::call_core(
        &mut *fixture.core.lock().await,
        "cli",
        &fixture.cli,
        Call::Cancel(JobKey {
            run_id: first.clone(),
        }),
    );
    assert!(task1.await.unwrap().is_err());
    assert_eq!(task2.await.unwrap().unwrap(), json!("A:B:C:hello"));
    fixture.drained().await;
    let core = fixture.core.lock().await;
    assert!(core
        .service_diagnostics(&second)
        .unwrap()
        .iter()
        .all(|call| call.state == "succeeded"));
    assert!(core
        .service_diagnostics(&first)
        .unwrap()
        .iter()
        .all(|call| call.state == "failed"));
}

#[tokio::test]
async fn intermediate_revocation_reaches_actual_c_before_any_further_effect() {
    let fixture = Fixture::new().await;
    let (root, task) = fixture.run("wait").await;
    fixture.wait_running_c(&root).await;
    Fixture::call_core(
        &mut *fixture.core.lock().await,
        "manager",
        &fixture.manager,
        Call::Revoke(PermissionKey {
            plugin_id: "fixture-b".into(),
            command_id: "svc:transform:run".into(),
            target: GrantTarget::Cli,
        }),
    );
    assert!(task.await.unwrap().is_err());
    fixture.drained().await;
    assert!(fixture
        .core
        .lock()
        .await
        .service_diagnostics(&root)
        .unwrap()
        .iter()
        .any(|call| call.failure_code == Some(ErrorCode::GrantRevoked)));
    assert_eq!(
        fixture
            .backend
            .reads
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
}

#[tokio::test]
async fn restart_requires_matching_snapshot_and_retains_old_call_diagnostics() {
    let fixture = Fixture::new().await;
    let (root, task) = fixture.run("echo").await;
    task.await.unwrap().unwrap();
    let plan = fixture.update_plan().await;
    let mut core = fixture.core.lock().await;
    core.begin_provider_change(plan.clone(), &plan.digest, false)
        .unwrap();
    core.commit_provider_change(&plan.digest, |_| Ok(()))
        .unwrap();
    drop(core);
    let tokens = || HashMap::from([("cli".into(), "local-cli".into())]);
    drop(fixture.core);
    let old = BuiltinCatalog::from_host_manifests(fixture.manifests[..3].to_vec()).unwrap();
    assert_eq!(
        RuntimeCore::managed_catalog(
            tokens(),
            DataStore::open(&fixture.profile.join("runtime.sqlite")).unwrap(),
            false,
            false,
            old
        )
        .err(),
        Some(ErrorCode::RecoveryPending)
    );
    let new = BuiltinCatalog::from_host_manifests(vec![
        fixture.manifests[0].clone(),
        fixture.manifests[1].clone(),
        fixture.manifests[3].clone(),
    ])
    .unwrap();
    let restarted = RuntimeCore::managed_catalog(
        tokens(),
        DataStore::open(&fixture.profile.join("runtime.sqlite")).unwrap(),
        false,
        false,
        new,
    )
    .unwrap();
    assert!(restarted
        .service_diagnostics(&root)
        .unwrap()
        .iter()
        .all(|call| call.provider.version == "1.0.0" && call.state == "succeeded"));
}

struct Fixture {
    _guard: tokio::sync::MutexGuard<'static, ()>,
    core: Core,
    profile: PathBuf,
    backend: Arc<service_runner::RunnerBackend>,
    manager: SessionProof,
    cli: SessionProof,
    manifests: Vec<Value>,
}

#[tokio::test]
async fn timed_out_provider_drains_its_actual_owned_descendant() {
    let fixture = Fixture::new().await;
    let (_, task) = fixture.run("daemon").await;
    assert!(task.await.unwrap().is_err());
    fixture.drained().await;
    assert!(
        fixture
            .backend
            .largest_group
            .load(std::sync::atomic::Ordering::SeqCst)
            > 1
    );
}

#[tokio::test]
async fn incompatible_or_cyclic_update_fails_before_draining_the_current_provider() {
    let fixture = Fixture::new().await;
    let mut incompatible = fixture.manifests[3].clone();
    incompatible["services"][0]["version"] = json!("2.0.0");
    let candidate = |provider| {
        vec![
            fixture.manifests[0].clone(),
            fixture.manifests[1].clone(),
            provider,
        ]
    };
    assert!(fixture
        .core
        .lock()
        .await
        .stage_provider_update(
            "fixture-c",
            BuiltinCatalog::from_host_manifests(candidate(incompatible.clone())).unwrap()
        )
        .is_err());
    incompatible = fixture.manifests[3].clone();
    incompatible["dependencies"]["services"] =
        fixture.manifests[0]["dependencies"]["services"].clone();
    assert_eq!(
        fixture
            .core
            .lock()
            .await
            .stage_provider_update(
                "fixture-c",
                BuiltinCatalog::from_host_manifests(candidate(incompatible)).unwrap()
            )
            .err(),
        Some(ErrorCode::DependencyCycle)
    );
    let (_, task) = fixture.run("echo").await;
    assert_eq!(task.await.unwrap().unwrap(), json!("A:B:C:hello"));
}

#[tokio::test]
async fn root_service_call_budget_cannot_be_reset_at_the_next_provider() {
    let fixture = Fixture::new().await;
    let mut grant = Fixture::grant(&fixture.manifests[0]);
    grant.max_calls = 1;
    Fixture::call_core(
        &mut *fixture.core.lock().await,
        "manager",
        &fixture.manager,
        Call::Grant(grant),
    );
    let (root, task) = fixture.run("allowed").await;
    assert!(task.await.unwrap().is_err());
    fixture.drained().await;
    assert_eq!(
        fixture
            .backend
            .reads
            .load(std::sync::atomic::Ordering::SeqCst),
        0
    );
    assert_eq!(
        fixture
            .core
            .lock()
            .await
            .service_diagnostics(&root)
            .unwrap()
            .len(),
        1
    );
}
impl Fixture {
    async fn new() -> Self {
        // Each fixture still tests concurrent roots internally; all assertions run.
        static TEST_PROFILES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
        let guard = TEST_PROFILES.lock().await;
        let profile =
            std::env::temp_dir().join(format!("flowtools-validation-g4-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&profile).unwrap();
        let directory = std::fs::canonicalize(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../packages/plugin-runner/.generated/g4-services"),
        )
        .expect("Build the pinned fixture artifacts before native tests");
        let manifests: Vec<Value> =
            serde_json::from_slice(&std::fs::read(directory.join("manifests.json")).unwrap())
                .unwrap();
        let catalog = BuiltinCatalog::from_host_manifests(manifests[..3].to_vec()).unwrap();
        let store = DataStore::open(&profile.join("runtime.sqlite")).unwrap();
        let mut core = RuntimeCore::managed_catalog(
            HashMap::from([
                ("manager".into(), "local-manager".into()),
                ("cli".into(), "local-cli".into()),
            ]),
            store,
            false,
            false,
            catalog,
        )
        .unwrap();
        let open = |core: &mut RuntimeCore, connection: &str, token: &str| {
            let response = core.handle(
                connection,
                Request {
                    version: PROTOCOL_MAJOR,
                    request_id: "open".into(),
                    session: None,
                    call: Call::Open(OpenSession {
                        token: token.into(),
                        client_version: CLIENT_VERSION.into(),
                        expected_instance_id: None,
                    }),
                },
            );
            let Outcome::Session(proof) = response.outcome else {
                panic!("Session")
            };
            proof
        };
        let manager = open(&mut core, "manager", "manager");
        let cli = open(&mut core, "cli", "cli");
        let grants = manifests[..3].iter().map(Self::grant).collect();
        Self::call_core(
            &mut core,
            "manager",
            &manager,
            Call::PolicyImport(PolicyImport {
                format_version: 1,
                cold_start: false,
                grants,
            }),
        );
        let mut config: Value = serde_json::from_str(runner_config()).unwrap();
        let runner = directory.join("fixture-runner.js");
        let path = runner
            .to_string_lossy()
            .trim_start_matches(r"\\?\")
            .to_string();
        config["runner"] = json!({"path":path,"sha256":format!("{:x}",Sha256::digest(std::fs::read(&runner).unwrap()))});
        let mut backend = service_runner::RunnerBackend::new(config);
        let allowed = profile.join("allowed.txt");
        let extra = profile.join("extra.txt");
        std::fs::write(&allowed, "granted").unwrap();
        std::fs::write(&extra, "PRIVATE_EXTRA_FILE").unwrap();
        backend.handles = BTreeMap::from([("allowed".into(), allowed), ("extra".into(), extra)]);
        Self {
            _guard: guard,
            core: Arc::new(Mutex::new(core)),
            profile,
            backend: Arc::new(backend),
            manager,
            cli,
            manifests,
        }
    }
    fn grant(manifest: &Value) -> PermissionGrant {
        let id = manifest["id"].as_str().unwrap();
        PermissionGrant {
            plugin_id: id.into(),
            command_id: if id == "fixture-a" {
                "run"
            } else {
                "svc:transform:run"
            }
            .into(),
            target: GrantTarget::Cli,
            package_digest: digest(manifest),
            effects: vec!["file-read".into()],
            scopes: if id == "fixture-a" {
                vec![Scope::FileHandle("allowed".into())]
            } else {
                vec![
                    Scope::FileHandle("allowed".into()),
                    Scope::FileHandle("extra".into()),
                ]
            },
            expires_at: now() + 60_000.0,
            max_calls: 8,
            cold_start: false,
            background: false,
        }
    }
    fn call_core(
        core: &mut RuntimeCore,
        connection: &str,
        proof: &SessionProof,
        call: Call,
    ) -> Outcome {
        let response = core.handle(
            connection,
            Request {
                version: PROTOCOL_MAJOR,
                request_id: uuid::Uuid::new_v4().to_string(),
                session: Some(proof.clone()),
                call,
            },
        );
        if let Outcome::Error(error) = response.outcome {
            panic!("Unexpected fixture error {:?}", error.code);
        }
        response.outcome
    }
    async fn run(&self, mode: &str) -> (String, tokio::task::JoinHandle<Result<Value, ErrorCode>>) {
        let mut core = self.core.lock().await;
        let result = Self::call_core(
            &mut core,
            "cli",
            &self.cli,
            Call::Submit(SubmitJob {
                plugin_id: "fixture-a".into(),
                command_id: "run".into(),
                input: json!({"text":"hello","mode":mode}),
                idempotency_key: uuid::Uuid::new_v4().to_string(),
                background: false,
                deadline: now() + 9500.0,
            }),
        );
        let Outcome::Receipt(receipt) = result else {
            panic!("Receipt")
        };
        let spec = core.take_run(&receipt.run_id).unwrap();
        let core = self.core.clone();
        let backend = self.backend.clone();
        let workspace = self.profile.join(&spec.run_id);
        let task = tokio::spawn(async move {
            let result =
                service_runner::execute(spec.clone(), None, core.clone(), workspace, backend).await;
            core.lock().await.finish(&spec.run_id, result.clone());
            result
        });
        (receipt.run_id, task)
    }
    async fn wait_running_c(&self, root: &str) {
        self.wait_running_provider(root, "fixture-c").await;
    }
    async fn wait_running_provider(&self, root: &str, provider: &str) {
        // Readiness is bounded by the existing Host deadline, not a startup SLA.
        // A terminal/expired root fails immediately; no retry or deadline extension.
        loop {
            let core = self.core.lock().await;
            core.check_run(root)
                .expect("Root must remain valid until the provider is running");
            if core
                .service_diagnostics(root)
                .unwrap()
                .iter()
                .any(|c| c.provider.id == provider && c.state == "running")
            {
                break;
            }
            drop(core);
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    async fn drained(&self) {
        tokio::time::timeout(Duration::from_secs(5), async {
            while self.core.lock().await.active_service_calls() != 0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
    }
    async fn update_plan(&self) -> ProviderChangePlan {
        self.core
            .lock()
            .await
            .stage_provider_update(
                "fixture-c",
                BuiltinCatalog::from_host_manifests(vec![
                    self.manifests[0].clone(),
                    self.manifests[1].clone(),
                    self.manifests[3].clone(),
                ])
                .unwrap(),
            )
            .unwrap()
    }
}

#[tokio::test]
async fn real_a_b_c_returns_and_denies_borrowed_files_without_leaking_provider_errors() {
    let fixture = Fixture::new().await;
    let (root, task) = fixture.run("echo").await;
    assert_eq!(task.await.unwrap().unwrap(), json!("A:B:C:hello"));
    let diagnostics = fixture
        .core
        .lock()
        .await
        .service_diagnostics(&root)
        .unwrap();
    assert_eq!(diagnostics.len(), 2);
    let b = diagnostics
        .iter()
        .find(|c| c.provider.id == "fixture-b")
        .unwrap();
    let c = diagnostics
        .iter()
        .find(|c| c.provider.id == "fixture-c")
        .unwrap();
    assert_eq!(b.parent_run_id, root);
    assert_eq!(c.parent_run_id, b.run_id);
    assert_eq!(c.root_run_id, root);
    assert_eq!(c.root_caller, "local-cli");
    assert!(diagnostics
        .iter()
        .all(|c| c.state == "succeeded" && c.dependency_lock == b.dependency_lock));
    let (_, task) = fixture.run("allowed").await;
    assert_eq!(task.await.unwrap().unwrap(), json!("A:B:granted"));
    let prior = fixture
        .backend
        .reads
        .load(std::sync::atomic::Ordering::SeqCst);
    let (root, task) = fixture.run("extra").await;
    assert!(task.await.unwrap().is_err());
    assert_eq!(
        fixture
            .backend
            .reads
            .load(std::sync::atomic::Ordering::SeqCst),
        prior
    );
    assert!(fixture
        .core
        .lock()
        .await
        .service_diagnostics(&root)
        .unwrap()
        .iter()
        .any(|c| c.provider.id == "fixture-c" && c.failure_code == Some(ErrorCode::ScopeDenied)));
    let (root, task) = fixture.run("fail").await;
    assert!(task.await.unwrap().is_err());
    let diagnostic = fixture
        .core
        .lock()
        .await
        .service_diagnostics(&root)
        .unwrap();
    assert!(diagnostic.iter().any(
        |c| c.provider.id == "fixture-c" && c.failure_code == Some(ErrorCode::ExecutionFailed)
    ));
    let text = serde_json::to_string(&diagnostic).unwrap();
    assert!(!text.contains("PRIVATE_PROVIDER_SECRET"));
    assert!(!text.contains("PRIVATE_EXTRA_FILE"));
    assert!(!text.contains("hello"));
    assert!(!text.contains(&fixture.profile.to_string_lossy().to_string()));
}

#[tokio::test]
async fn actual_cancellation_drains_both_hops_and_update_waits_for_owned_processes() {
    let fixture = Fixture::new().await;
    let (root, task) = fixture.run("wait").await;
    fixture.wait_running_c(&root).await;
    let plan = fixture.update_plan().await;
    let mut core = fixture.core.lock().await;
    core.begin_provider_change(plan.clone(), &plan.digest, false)
        .unwrap();
    assert_eq!(
        core.begin_provider_change(plan.clone(), &plan.digest, false),
        Err(ErrorCode::RuntimeBusy)
    );
    assert_eq!(
        core.commit_provider_change(&plan.digest, |_| panic!("Migration before real drain")),
        Err(ErrorCode::RuntimeBusy)
    );
    Fixture::call_core(
        &mut core,
        "cli",
        &fixture.cli,
        Call::Cancel(JobKey {
            run_id: root.clone(),
        }),
    );
    drop(core);
    assert!(matches!(
        task.await.unwrap(),
        Err(ErrorCode::Aborted) | Err(ErrorCode::SessionInvalid)
    ));
    fixture.drained().await;
    let mut core = fixture.core.lock().await;
    core.commit_provider_change(&plan.digest, |tx| {
        tx.execute(
            "INSERT INTO core_metadata(key,value) VALUES('fixture-migration','1.1.0')",
            [],
        )
        .unwrap();
        Ok(())
    })
    .unwrap();
    Fixture::call_core(
        &mut core,
        "manager",
        &fixture.manager,
        Call::Grant(Fixture::grant(&fixture.manifests[3])),
    );
    drop(core);
    let (new_root, task) = fixture.run("echo").await;
    assert_eq!(task.await.unwrap().unwrap(), json!("A:B:C:hello"));
    let core = fixture.core.lock().await;
    let old = core.service_diagnostics(&root).unwrap();
    let new = core.service_diagnostics(&new_root).unwrap();
    assert!(old.iter().all(|c| c.state == "failed"));
    assert!(old.iter().all(|c| c.provider.version == "1.0.0"));
    assert!(new
        .iter()
        .any(|c| c.provider.id == "fixture-c" && c.provider.version == "1.1.0"));
    assert_ne!(old[0].dependency_lock, new[0].dependency_lock);
}

#[tokio::test]
async fn migration_failure_and_unload_confirmation_preserve_accepted_jobs() {
    let fixture = Fixture::new().await;
    let (root, task) = fixture.run("echo").await;
    task.await.unwrap().unwrap();
    let plan = fixture.update_plan().await;
    let mut core = fixture.core.lock().await;
    core.begin_provider_change(plan.clone(), &plan.digest, false)
        .unwrap();
    assert_eq!(
        core.commit_provider_change(&plan.digest, |tx| {
            tx.execute(
                "INSERT INTO core_metadata(key,value) VALUES('should-rollback','secret')",
                [],
            )
            .unwrap();
            Err(ErrorCode::StorageFailed)
        }),
        Err(ErrorCode::StorageFailed)
    );
    assert_eq!(
        core.provider_unload_plan("fixture-c")
            .unwrap()
            .provider
            .version,
        "1.0.0"
    );
    let plan = core.provider_unload_plan("fixture-c").unwrap();
    assert_eq!(plan.consumers.len(), 2);
    assert_eq!(
        core.begin_provider_change(plan.clone(), &plan.digest, false),
        Err(ErrorCode::DependencyConflict)
    );
    assert_eq!(
        core.begin_provider_change(plan.clone(), "unreviewed", true),
        Err(ErrorCode::ApprovalRequired)
    );
    drop(core);
    let (waiting, task) = fixture.run("wait").await;
    fixture.wait_running_c(&waiting).await;
    let mut core = fixture.core.lock().await;
    assert_eq!(
        core.begin_provider_change(plan.clone(), &plan.digest, true),
        Err(ErrorCode::ApprovalRequired)
    );
    let plan = core.provider_unload_plan("fixture-c").unwrap();
    core.begin_provider_change(plan.clone(), &plan.digest, true)
        .unwrap();
    assert_eq!(
        core.commit_provider_change(&plan.digest, |_| panic!("Unloading accepted work")),
        Err(ErrorCode::RuntimeBusy)
    );
    Fixture::call_core(
        &mut core,
        "cli",
        &fixture.cli,
        Call::Cancel(JobKey { run_id: waiting }),
    );
    drop(core);
    assert!(task.await.unwrap().is_err());
    fixture.drained().await;
    let mut core = fixture.core.lock().await;
    core.commit_provider_change(&plan.digest, |_| Ok(()))
        .unwrap();
    assert_eq!(core.service_diagnostics(&root).unwrap().len(), 2);
    let response = core.handle(
        "cli",
        Request {
            version: PROTOCOL_MAJOR,
            request_id: "disabled".into(),
            session: Some(fixture.cli.clone()),
            call: Call::Submit(SubmitJob {
                plugin_id: "fixture-a".into(),
                command_id: "run".into(),
                input: json!({"text":"hello"}),
                idempotency_key: "after-unload".into(),
                background: false,
                deadline: now() + 9000.0,
            }),
        },
    );
    assert!(matches!(
        response.outcome,
        Outcome::Error(RuntimeError {
            code: ErrorCode::DependencyMissing
        })
    ));
}
