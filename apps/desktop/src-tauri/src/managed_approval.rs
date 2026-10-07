//! Native consent text. Credentials and business payloads never belong here.
use flowtools_runtime_core::protocol::*;

const COLUMNS: usize = 56;
const PAGE_LINES: usize = 24;

fn yes(value: bool) -> &'static str {
    if value {
        "允许（true）"
    } else {
        "不允许（false）"
    }
}
fn quoted(value: &str) -> String {
    // Quote control characters so request strings cannot forge consent labels.
    serde_json::to_string(value).expect("string serialization")
}
fn field(lines: &mut Vec<String>, label: &str, value: impl std::fmt::Display) {
    let mut line = String::new();
    let mut columns = 0;
    for character in format!("{label}：{value}").chars() {
        let width = if character.is_ascii() { 1 } else { 2 };
        if columns + width > COLUMNS {
            lines.push(std::mem::take(&mut line));
            columns = 0;
        }
        line.push(character);
        columns += width;
    }
    lines.push(line);
}
fn pages(lines: Vec<String>, group: usize, total: usize) -> Vec<String> {
    let count = lines.len().div_ceil(PAGE_LINES);
    lines.chunks(PAGE_LINES).enumerate().map(|(index, page)| {
        format!("授权审阅 {group}/{total} · 第 {}/{count} 页\n{}\n确认全部页面后才生效；取消任一页不导入。", index + 1, page.join("\n"))
    }).collect()
}
fn grant_lines(
    grant: &PermissionGrant,
    targets: &[GrantTarget],
    cold: Option<bool>,
) -> Result<Vec<String>, ErrorCode> {
    let mut lines = vec!["批准命令授权；此操作不执行命令。".into()];
    if let Some(cold) = cold {
        field(&mut lines, "连接时按需启动 Runtime coldStart", yes(cold));
    }
    field(
        &mut lines,
        "调用方",
        targets
            .iter()
            .map(GrantTarget::caller)
            .collect::<Vec<_>>()
            .join("、"),
    );
    field(&mut lines, "插件", quoted(&grant.plugin_id));
    field(&mut lines, "命令", quoted(&grant.command_id));
    field(&mut lines, "包摘要 SHA-256", quoted(&grant.package_digest));
    let effects = grant
        .effects
        .iter()
        .map(|effect| match effect.as_str() {
            "data-read" => "读取插件数据（data-read）".into(),
            "data-write" => "写入插件数据（data-write）".into(),
            _ => quoted(effect),
        })
        .collect::<Vec<String>>();
    field(
        &mut lines,
        "请求效果 effects",
        if effects.is_empty() {
            "无".into()
        } else {
            effects.join("、")
        },
    );
    if grant.scopes.is_empty() {
        field(&mut lines, "范围 scopes", "无");
    }
    for scope in &grant.scopes {
        if let flowtools_runtime_core::broker::Scope::PluginData { key_prefix } = scope {
            field(&mut lines, "范围", "仅此插件数据（plugin-data）");
            field(&mut lines, "数据键前缀 key_prefix", quoted(key_prefix));
        } else {
            field(
                &mut lines,
                "范围 scope",
                serde_json::to_string(scope).map_err(|_| ErrorCode::InvalidRequest)?,
            );
        }
    }
    if !grant.expires_at.is_finite() {
        return Err(ErrorCode::InvalidRequest);
    }
    let expires = jiff::Timestamp::from_millisecond(grant.expires_at as i64)
        .map_err(|_| ErrorCode::InvalidRequest)?;
    field(&mut lines, "到期时间 UTC", expires);
    field(&mut lines, "到期 Unix 毫秒 expiresAt", grant.expires_at);
    field(&mut lines, "每次运行调用上限 maxCalls", grant.max_calls);
    field(&mut lines, "命令冷启动 coldStart", yes(grant.cold_start));
    field(
        &mut lines,
        "窗口关闭后运行 background",
        yes(grant.background),
    );
    Ok(lines)
}

pub fn runtime_prompts(call: &Call, initialize: bool) -> Result<Vec<String>, ErrorCode> {
    if initialize && !matches!(call, Call::PolicyImport(_)) {
        return Err(ErrorCode::SessionInvalid);
    }
    match call {
        Call::Stop => Ok(vec!["停止 Runtime？\n此操作会取消所有活动任务并等待清理。\n收到停止回执后，仍需确认写入者已释放。".into()]),
        Call::Policy(policy) => Ok(vec![format!("修改 Runtime 按需启动设置？\ncoldStart：{}\n不授予插件权限，也不执行命令。", yes(policy.cold_start))]),
        Call::Grant(grant) => Ok(pages(grant_lines(grant, std::slice::from_ref(&grant.target), None)?, 1, 1)),
        Call::PolicyImport(policy) => {
            if policy.format_version != 1 || policy.grants.len() > 32 { return Err(ErrorCode::InvalidRequest); }
            if policy.grants.is_empty() {
                return Ok(vec![format!("{} Runtime？\n连接时按需启动 coldStart：{}\n本次没有插件授权，不执行业务任务。", if initialize { "初始化" } else { "修改" }, yes(policy.cold_start))]);
            }
            // Only identical grants share a prompt; caller targets stay explicit.
            let mut groups: Vec<(serde_json::Value, &PermissionGrant, Vec<GrantTarget>)> = Vec::new();
            for grant in &policy.grants {
                let mut value = serde_json::to_value(grant).map_err(|_| ErrorCode::InvalidRequest)?;
                value.as_object_mut().ok_or(ErrorCode::InvalidRequest)?.remove("target");
                if let Some((_, _, targets)) = groups.iter_mut().find(|(key, _, _)| *key == value) {
                    targets.push(grant.target.clone());
                } else { groups.push((value, grant, vec![grant.target.clone()])); }
            }
            let mut prompts = Vec::new();
            for (index, (_, grant, targets)) in groups.iter().enumerate() {
                let mut lines = grant_lines(grant, targets, Some(policy.cold_start))?;
                if initialize {
                    lines[0] = "初始化 Runtime 并批准以下授权；不执行命令。".into();
                }
                prompts.extend(pages(lines, index + 1, groups.len()));
            }
            Ok(prompts)
        }
        _ => Err(ErrorCode::SessionInvalid),
    }
}

pub fn storage_prompt(action: &StorageAction) -> Result<Option<String>, ErrorCode> {
    Ok(match action {
        StorageAction::List => None,
        StorageAction::Create => Some("创建备份？\n请先停止 Runtime。\n复制当前数据为离线备份；不修改数据或授权。".into()),
        StorageAction::Restore { backup_id } => Some(format!("恢复所选备份？\n备份 UUID：{backup_id}\n请先停止 Runtime。\n数据将回滚到这份备份，全部授权撤销。\n未完成任务将中断；原库和私有任务正文保留。\n不会自动重放任务；核对数据后重新批准。")),
        StorageAction::Retry => Some("继续未完成的恢复？\n请先停止 Runtime。\n继续原恢复记录，不另建恢复操作。\n数据将回滚，全部授权撤销，未完成任务中断。\n原库和私有任务正文保留；不会自动重放任务。".into()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flowtools_runtime_core::broker::Scope;

    fn grant(target: GrantTarget) -> PermissionGrant {
        PermissionGrant {
            plugin_id: "plugin-todo-list".into(),
            command_id: "run".into(),
            target,
            package_digest: "a".repeat(64),
            effects: vec!["data-read".into(), "data-write".into()],
            scopes: vec![Scope::PluginData {
                key_prefix: "todos".into(),
            }],
            expires_at: 1_791_374_000_000.0,
            max_calls: 128,
            cold_start: true,
            background: false,
        }
    }

    #[test]
    fn consent_shows_every_grant_field_with_both_callers_without_a_long_json_line() {
        let prompts = runtime_prompts(
            &Call::PolicyImport(PolicyImport {
                format_version: 1,
                cold_start: true,
                grants: vec![grant(GrantTarget::Cli), grant(GrantTarget::Desktop)],
            }),
            false,
        )
        .unwrap();
        assert_eq!(prompts.len(), 1);
        let text = prompts.join("\n");
        for field in [
            "plugin-todo-list",
            "run",
            "local-cli",
            "local-desktop",
            "data-read",
            "data-write",
            "todos",
            "1791374000000",
            "128",
            "background",
            "false",
            "coldStart",
            "true",
        ] {
            assert!(text.contains(field), "missing {field}");
        }
        assert!(text.replace('\n', "").contains(&"a".repeat(64)));
        assert!(text.contains("UTC"));
        assert!(!text.contains("停止 Runtime"));
        assert!(!text.contains("token"));
        assert!(text.lines().all(|line| line.chars().count() <= 56));
    }

    #[test]
    fn operations_explain_their_own_effects_and_never_display_business_credentials() {
        let init = runtime_prompts(
            &Call::PolicyImport(PolicyImport {
                format_version: 1,
                cold_start: false,
                grants: vec![],
            }),
            true,
        )
        .unwrap()
        .join("\n");
        assert!(init.contains("初始化"));
        assert!(!init.contains("停止 Runtime"));
        let stop = runtime_prompts(&Call::Stop, false).unwrap().join("\n");
        assert!(stop.contains("取消所有活动任务"));
        let create = storage_prompt(&StorageAction::Create).unwrap().unwrap();
        assert!(create.contains("创建备份"));
        assert!(!create.contains("回滚"));
        assert!(!create.contains("撤销"));
        let restore = storage_prompt(&StorageAction::Restore {
            backup_id: "55d5f410-714b-4622-b68c-18fb20527467".into(),
        })
        .unwrap()
        .unwrap();
        for field in [
            "55d5f410-714b-4622-b68c-18fb20527467",
            "回滚",
            "撤销",
            "中断",
            "保留",
            "不会自动重放",
        ] {
            assert!(restore.contains(field));
        }
        assert!(storage_prompt(&StorageAction::List).unwrap().is_none());
        assert_eq!(
            runtime_prompts(
                &Call::Open(OpenSession {
                    token: "secret-never-render".into(),
                    client_version: CLIENT_VERSION.into(),
                    expected_instance_id: None
                }),
                false
            ),
            Err(ErrorCode::SessionInvalid)
        );
    }

    #[test]
    fn distinct_background_rights_are_not_merged_and_long_values_are_reviewed_in_pages() {
        let first = grant(GrantTarget::Cli);
        let mut second = grant(GrantTarget::Desktop);
        second.background = true;
        second.scopes = vec![Scope::PluginData {
            key_prefix: "todos-".repeat(400),
        }];
        let prompts = runtime_prompts(
            &Call::PolicyImport(PolicyImport {
                format_version: 1,
                cold_start: true,
                grants: vec![first, second],
            }),
            false,
        )
        .unwrap();
        assert!(prompts.len() > 2);
        assert!(prompts[0].contains("local-cli"));
        assert!(!prompts[0].contains("local-desktop"));
        assert!(prompts[0].contains("不允许（false）"));
        assert!(prompts.last().unwrap().contains("允许（true）"));
        assert!(prompts
            .iter()
            .all(|prompt| prompt.lines().count() <= PAGE_LINES + 2));
        let bodies = prompts
            .iter()
            .map(|prompt| {
                prompt
                    .lines()
                    .skip(1)
                    .take(prompt.lines().count() - 2)
                    .collect::<String>()
            })
            .collect::<String>();
        assert!(bodies.contains(&quoted(&"todos-".repeat(400))));
    }

    #[test]
    fn request_strings_cannot_insert_fake_consent_labels() {
        let mut requested = grant(GrantTarget::Cli);
        requested.plugin_id = "plugin-todo-list\n调用方：administrator".into();
        let prompts = runtime_prompts(&Call::Grant(requested), false).unwrap();
        assert!(prompts.join("\n").contains("\\n"));
        assert!(!prompts
            .join("\n")
            .lines()
            .any(|line| line.starts_with("调用方：administrator")));
        assert_eq!(
            runtime_prompts(
                &Call::PolicyImport(PolicyImport {
                    format_version: 2,
                    cold_start: true,
                    grants: vec![]
                }),
                false
            ),
            Err(ErrorCode::InvalidRequest)
        );
    }
}
