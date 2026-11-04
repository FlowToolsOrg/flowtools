use super::*;
use flowtools_runtime_core::{
    catalog::BuiltinCatalog,
    file_workflows::media_io::{
        CreateMediaPlan, MediaPlanPreview, StartMediaPlan, UserMediaAction,
    },
};
use std::collections::HashMap;
#[path = "../test/fixtures/media-candidate/video_input.rs"]
mod video_input;

struct Fixture {
    root: PathBuf,
    profile: PathBuf,
    core: Core,
    manager: SessionProof,
    cli: SessionProof,
}
impl Fixture {
    async fn new() -> Self {
        let root =
            std::env::temp_dir().join(format!("flowtools-g7-media-{}", uuid::Uuid::new_v4()));
        let profile = root.join("profile");
        std::fs::create_dir_all(&profile).unwrap();
        let mut store = DataStore::open(&profile.join("runtime.sqlite")).unwrap();
        let trust =
            flowtools_runtime_core::packages::tools::builtin::ensure_installed(&mut store).unwrap();
        let mut core = RuntimeCore::managed_tool_catalog(
            HashMap::from([
                ("manager-key".into(), "local-manager".into()),
                ("cli-key".into(), "local-cli".into()),
            ]),
            store,
            false,
            false,
            BuiltinCatalog::embedded_with_media(),
            trust,
        )
        .unwrap();
        let mut open = |connection: &str, token: &str| {
            let outcome = core
                .handle(
                    connection,
                    Request {
                        version: 1,
                        request_id: uuid::Uuid::new_v4().to_string(),
                        session: None,
                        call: Call::Open(OpenSession {
                            token: token.into(),
                            client_version: CLIENT_VERSION.into(),
                            expected_instance_id: None,
                        }),
                    },
                )
                .outcome;
            let Outcome::Session(session) = outcome else {
                panic!("Session: {outcome:?}")
            };
            session
        };
        let manager = open("manager", "manager-key");
        let cli = open("cli", "cli-key");
        Self {
            root,
            profile,
            core: Arc::new(Mutex::new(core)),
            manager,
            cli,
        }
    }
    async fn call(&self, call: Call) -> Outcome {
        self.core
            .lock()
            .await
            .handle(
                "manager",
                Request {
                    version: 1,
                    request_id: uuid::Uuid::new_v4().to_string(),
                    session: Some(self.manager.clone()),
                    call,
                },
            )
            .outcome
    }
    async fn plan(
        &self,
        input: &str,
        output: Option<&str>,
        action: UserMediaAction,
    ) -> MediaPlanPreview {
        let outcome = self
            .call(Call::MediaPlanCreate(CreateMediaPlan {
                request_key: uuid::Uuid::new_v4().to_string(),
                input_path: self.root.join(input).to_str().unwrap().into(),
                output_path: output.map(|n| self.root.join(n).to_str().unwrap().into()),
                action,
            }))
            .await;
        let Outcome::MediaPlan(plan) = outcome else {
            panic!("Plan: {outcome:?}")
        };
        plan
    }
    async fn accept(&self, plan: &MediaPlanPreview) -> String {
        let outcome = self
            .call(Call::MediaStart(StartMediaPlan {
                id: plan.id.clone(),
                digest: plan.digest.clone(),
                target: GrantTarget::Cli,
            }))
            .await;
        let Outcome::Receipt(receipt) = outcome else {
            panic!("Start: {outcome:?}")
        };
        receipt.run_id
    }
    async fn execute(&self, run: &str) -> Result<Value, ErrorCode> {
        let spec = self.core.lock().await.take_run(run).unwrap();
        let result = service_runner::execute(
            spec,
            None,
            self.core.clone(),
            self.profile.join(run),
            Arc::new(service_runner::RunnerBackend::fixed().unwrap()),
        )
        .await;
        self.core.lock().await.finish(run, result.clone());
        result
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let core = std::mem::replace(
            &mut self.core,
            Arc::new(Mutex::new(RuntimeCore::validation(
                "cleanup-cli".into(),
                "cleanup-desktop".into(),
            ))),
        );
        drop(core);
        assert_eq!(self.root.parent().unwrap(), std::env::temp_dir());
        assert!(self
            .root
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("flowtools-g7-media-"));
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
fn wave() -> Vec<u8> {
    let data: Vec<u8> = (0..16000)
        .flat_map(|n| (((n % 257) as i16 - 128) * 128).to_le_bytes())
        .collect();
    let mut b = b"RIFF".to_vec();
    b.extend_from_slice(&(36 + data.len() as u32).to_le_bytes());
    b.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
    b.extend_from_slice(&16000u32.to_le_bytes());
    b.extend_from_slice(&32000u32.to_le_bytes());
    b.extend_from_slice(&2u16.to_le_bytes());
    b.extend_from_slice(&16u16.to_le_bytes());
    b.extend_from_slice(b"data");
    b.extend_from_slice(&(data.len() as u32).to_le_bytes());
    b.extend(data);
    b
}

#[tokio::test]
async fn actual_compiled_plugins_share_signed_tools_and_publish_playable_outputs() {
    let f = Fixture::new().await;
    std::fs::write(f.root.join("input.wav"), wave()).unwrap();
    let info = f.plan("input.wav", None, UserMediaAction::Inspect).await;
    let run = f.accept(&info).await;
    let metadata = f.execute(&run).await.unwrap();
    assert_eq!(metadata["value"]["result"]["formats"], json!(["wav"]));
    let audio = f
        .plan("input.wav", Some("extracted.wav"), UserMediaAction::Audio)
        .await;
    let run = f.accept(&audio).await;
    let result = f.execute(&run).await.unwrap();
    assert_eq!(result["value"]["result"]["published"], true);
    let output = std::fs::read(f.root.join("extracted.wav")).unwrap();
    assert_eq!(&output[..4], b"RIFF");
    assert!(output.windows(4).any(|w| w == b"data"));
    assert_eq!(f.accept(&audio).await, run);
    assert_eq!(std::fs::read(f.root.join("extracted.wav")).unwrap(), output);
    std::fs::write(f.root.join("input.avi"), video_input::video(3, 16)).unwrap();
    let video = f
        .plan("input.avi", Some("converted.webm"), UserMediaAction::Webm)
        .await;
    let video_run = f.accept(&video).await;
    let result = f.execute(&video_run).await.unwrap();
    assert_eq!(
        result["value"]["result"]["formats"],
        json!(["matroska", "webm"])
    );
    assert_eq!(result["value"]["result"]["streams"][0]["codec"], "vp9");
    assert_eq!(
        &std::fs::read(f.root.join("converted.webm")).unwrap()[..4],
        &[0x1a, 0x45, 0xdf, 0xa3]
    );
    let inventory = f
        .core
        .lock()
        .await
        .handle(
            "cli",
            Request {
                version: 1,
                request_id: uuid::Uuid::new_v4().to_string(),
                session: Some(f.cli.clone()),
                call: Call::ToolInventory(
                    flowtools_runtime_core::tool_inventory::ToolInventoryQuery {
                        offset: 0,
                        snapshot: None,
                    },
                ),
            },
        )
        .outcome;
    let Outcome::ToolInventory(page) = inventory else {
        panic!("Inventory")
    };
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].id, "ffmpeg");
    assert_eq!(f.core.lock().await.active_media_inspections(), 0);
}

#[tokio::test]
async fn file_changes_collision_cancel_and_revocation_never_publish() {
    let f = Fixture::new().await;
    std::fs::write(f.root.join("input.wav"), wave()).unwrap();
    let p = f
        .plan("input.wav", Some("collision.wav"), UserMediaAction::Audio)
        .await;
    std::fs::write(f.root.join("collision.wav"), b"foreign-output").unwrap();
    let run = f.accept(&p).await;
    assert!(f.execute(&run).await.is_err());
    assert_eq!(
        std::fs::read(f.root.join("collision.wav")).unwrap(),
        b"foreign-output"
    );
    let p = f
        .plan("input.wav", Some("changed.wav"), UserMediaAction::Audio)
        .await;
    std::fs::write(f.root.join("input.wav"), b"changed").unwrap();
    let run = f.accept(&p).await;
    assert!(f.execute(&run).await.is_err());
    assert!(!f.root.join("changed.wav").exists());
    std::fs::write(f.root.join("input.wav"), wave()).unwrap();
    let p = f
        .plan("input.wav", Some("cancelled.wav"), UserMediaAction::Audio)
        .await;
    let run = f.accept(&p).await;
    f.core.lock().await.cancel(&run);
    assert!(!f.root.join("cancelled.wav").exists());
    let p = f
        .plan("input.wav", Some("revoked.wav"), UserMediaAction::Audio)
        .await;
    let run = f.accept(&p).await;
    f.call(Call::Revoke(PermissionKey {
        plugin_id: "plugin-media-converter".into(),
        command_id: "run".into(),
        target: GrantTarget::Cli,
    }))
    .await;
    assert!(f.core.lock().await.take_run(&run).is_none());
    assert!(!f.root.join("revoked.wav").exists());
}

#[tokio::test]
async fn active_native_cancel_and_revoke_drain_before_any_output_publication() {
    let f = Fixture::new().await;
    std::fs::write(f.root.join("input.avi"), video_input::video(16, 64)).unwrap();
    for revoke in [false, true] {
        let output = if revoke {
            "revoked-active.webm"
        } else {
            "cancelled-active.webm"
        };
        let p = f
            .plan("input.avi", Some(output), UserMediaAction::Webm)
            .await;
        let run = f.accept(&p).await;
        let spec = f.core.lock().await.take_run(&run).unwrap();
        let core = f.core.clone();
        let workspace = f.profile.join(&run);
        let running = run.clone();
        let worker = tokio::spawn(async move {
            let result = service_runner::execute(
                spec,
                None,
                core.clone(),
                workspace,
                Arc::new(service_runner::RunnerBackend::fixed().unwrap()),
            )
            .await;
            core.lock().await.finish(&running, result.clone());
            result
        });
        tokio::time::timeout(Duration::from_secs(5), async {
            while f.core.lock().await.active_media_inspections() == 0 {
                assert!(
                    !worker.is_finished(),
                    "Expected original native preparation before cancel"
                );
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap();
        if revoke {
            f.call(Call::Revoke(PermissionKey {
                plugin_id: "plugin-media-converter".into(),
                command_id: "run".into(),
                target: GrantTarget::Cli,
            }))
            .await;
        } else {
            f.core.lock().await.cancel(&run);
        }
        assert!(worker.await.unwrap().is_err());
        assert_eq!(f.core.lock().await.active_media_inspections(), 0);
        assert!(!f.root.join(output).exists());
    }
}
