use flowtools_runtime_core::protocol::{ErrorCode, RuntimeStatus};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tauri::{menu::MenuItem, AppHandle, Wry};

#[derive(Clone, Default)]
struct QueryGate(Arc<AtomicBool>);

struct QueryLease(Arc<AtomicBool>);

impl QueryGate {
    fn begin(&self) -> Option<QueryLease> {
        self.0
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .ok()
            .map(|_| QueryLease(self.0.clone()))
    }
}

impl Drop for QueryLease {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

struct Labels {
    status: String,
    checked_at: String,
}

fn snapshot_labels(result: Result<RuntimeStatus, ErrorCode>, checked_at: &str) -> Labels {
    let result = result.and_then(|status| {
        if status.active_jobs > status.jobs {
            Err(ErrorCode::InvalidResponse)
        } else {
            Ok(status)
        }
    });
    let status = match result {
        Ok(status) => {
            let mode = match status.mode.as_str() {
                "managed" => "运行模式",
                "management" => "仅管理模式",
                "validation" => "验证模式",
                _ => "未知模式",
            };
            format!(
                "Runtime 快照：{mode} · {} 活动 / {} 记录",
                status.active_jobs, status.jobs
            )
        }
        Err(code) => {
            // Select only the existing public enum code, never a raw error.
            let code = serde_json::to_value(code)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| "INVALID_RESPONSE".into());
            format!("Runtime：状态未确认 · {code}")
        }
    };
    Labels {
        status,
        checked_at: format!("查询时间：{checked_at}（不会自动刷新）"),
    }
}

pub(super) struct TrayStatus {
    pub(super) status: MenuItem<Wry>,
    pub(super) checked_at: MenuItem<Wry>,
    pub(super) query: MenuItem<Wry>,
    gate: QueryGate,
}

impl TrayStatus {
    pub(super) fn new(app: &AppHandle) -> tauri::Result<Self> {
        Ok(Self {
            status: MenuItem::with_id(
                app,
                "shell-runtime-snapshot",
                "Runtime：尚未查询",
                false,
                None::<&str>,
            )?,
            checked_at: MenuItem::with_id(
                app,
                "shell-runtime-snapshot-time",
                "仅查询已运行的服务，不会启动或初始化",
                false,
                None::<&str>,
            )?,
            query: MenuItem::with_id(
                app,
                "shell-runtime-status-refresh",
                "查询 Runtime 状态 · Prototype",
                true,
                None::<&str>,
            )?,
            gate: QueryGate::default(),
        })
    }

    pub(super) fn request(&self, app: &AppHandle) {
        let Some(lease) = self.gate.begin() else {
            return;
        };
        // Clear the old snapshot before any native connection. If the menu is
        // unavailable, do not start a query whose result cannot be displayed.
        if self
            .status
            .set_text("Runtime：正在查询…")
            .and_then(|()| self.checked_at.set_text("旧快照已清除；本次查询尚未完成"))
            .is_err()
        {
            let _ = self
                .status
                .set_text("Runtime：菜单更新失败，请从设置页查询");
            tauri_plugin_log::log::warn!("DESKTOP_RUNTIME_STATUS_MENU_UNAVAILABLE");
            return;
        }
        let app = app.clone();
        let status = self.status.clone();
        let checked_at = self.checked_at.clone();
        tauri::async_runtime::spawn(async move {
            let _lease = lease;
            let result = crate::managed_runtime::status_for_main_window(&app)
                .await
                .map_err(|error| error.code);
            let time = jiff::Zoned::try_from(std::time::SystemTime::now())
                .and_then(|value| jiff::fmt::strtime::format("%Y-%m-%d %H:%M:%S %:z", &value))
                .unwrap_or_else(|_| "时间不可用".into());
            let labels = snapshot_labels(result, &time);
            if status
                .set_text(labels.status)
                .and_then(|()| checked_at.set_text(labels.checked_at))
                .is_err()
            {
                let _ = status.set_text("Runtime：菜单更新失败，请从设置页查询");
                tauri_plugin_log::log::warn!("DESKTOP_RUNTIME_STATUS_MENU_UNAVAILABLE");
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHECKED_AT: &str = "2026-10-25 21:00:00 +08:00";

    fn status() -> RuntimeStatus {
        RuntimeStatus {
            instance_id: "PRIVATE_INSTANCE_NOT_FOR_MENU".into(),
            mode: "managed".into(),
            jobs: 12,
            active_jobs: 3,
        }
    }

    #[test]
    fn overlapping_requests_are_refused_until_the_owned_query_finishes() {
        let gate = QueryGate::default();
        let lease = gate.begin().unwrap();
        assert!(gate.clone().begin().is_none());
        drop(lease);
        assert!(gate.begin().is_some());
    }

    #[tokio::test]
    async fn cancelling_the_pending_native_query_releases_its_gate() {
        let gate = QueryGate::default();
        let pending_gate = gate.clone();
        let (started, ready) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let lease = pending_gate.begin().unwrap();
            started.send(()).unwrap();
            std::future::pending::<()>().await;
            drop(lease);
        });
        ready.await.unwrap();
        assert!(gate.begin().is_none());
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(gate.begin().is_some());
    }

    #[test]
    fn observed_labels_use_counts_known_modes_and_the_query_receipt_only() {
        let labels = snapshot_labels(Ok(status()), CHECKED_AT);
        assert_eq!(labels.status, "Runtime 快照：运行模式 · 3 活动 / 12 记录");
        assert_eq!(
            labels.checked_at,
            "查询时间：2026-10-25 21:00:00 +08:00（不会自动刷新）"
        );
        assert!(!labels.status.contains("PRIVATE_"));
        for (mode, expected) in [
            ("management", "仅管理模式"),
            ("validation", "验证模式"),
            ("PRIVATE_MODE_NOT_FOR_MENU", "未知模式"),
        ] {
            let mut value = status();
            value.mode = mode.into();
            let labels = snapshot_labels(Ok(value), CHECKED_AT);
            assert!(labels.status.contains(expected));
            assert!(!labels.status.contains("PRIVATE_"));
        }
    }

    #[test]
    fn failures_contain_only_the_stable_code_and_no_previous_snapshot_or_offline_claim() {
        let labels = snapshot_labels(Err(ErrorCode::RuntimeDisconnected), CHECKED_AT);
        assert_eq!(labels.status, "Runtime：状态未确认 · RUNTIME_DISCONNECTED");
        assert!(labels.checked_at.contains(CHECKED_AT));
        assert!(!labels.status.contains("离线"));
        assert!(!labels.status.contains("活动"));
        assert_eq!(
            snapshot_labels(Err(ErrorCode::SessionInvalid), CHECKED_AT).status,
            "Runtime：状态未确认 · SESSION_INVALID"
        );
    }

    #[test]
    fn impossible_active_counts_refuse_a_successful_snapshot() {
        let mut value = status();
        value.active_jobs = value.jobs + 1;
        assert_eq!(
            snapshot_labels(Ok(value), CHECKED_AT).status,
            "Runtime：状态未确认 · INVALID_RESPONSE"
        );
    }
}
