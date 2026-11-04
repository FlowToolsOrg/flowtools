use super::*;
use serde_json::json;

fn fixture(id: &str, max_calls: u32) -> (CapabilityBroker, CommandIdentity, Value, Grant) {
    let catalog = crate::catalog::BuiltinCatalog::embedded();
    let (manifest, command) = catalog.command("plugin-todo-list", "run").unwrap();
    let mut manifest = manifest.clone();
    let mut command = command.clone();
    manifest["id"] = json!(id);
    command["effects"] = json!(["tool-execute"]);
    command["permissions"] =
        json!([{"capability":"tool","operations":["execute"],"scopes":["lock:ffmpeg:inspect-v1"]}]);
    manifest["commands"][0] = command.clone();
    let identity = CommandIdentity::from_manifest("host-cli", &manifest, &command);
    let grant = Grant {
        effects: vec!["tool-execute".into()],
        scopes: vec![Scope::ToolLock {
            lock: "ffmpeg".into(),
            action: "inspect-v1".into(),
        }],
        expires_at: 200.0,
        max_calls,
    };
    (CapabilityBroker::default(), identity, command, grant)
}
fn operation() -> CapabilityOperation {
    CapabilityOperation::ToolExecute {
        lock: "ffmpeg".into(),
        action: "inspect-v1".into(),
    }
}
#[test]
fn the_charged_last_call_still_rechecks_each_phase_without_creating_another_call() {
    let (mut broker, identity, command, grant) = fixture("root", 1);
    broker.approve(identity.clone(), grant).unwrap();
    let session = broker.bind(identity.clone(), command, 100.0, 10.0).unwrap();
    let claim = broker
        .claim_tool_call(&session, &operation(), 11.0)
        .unwrap();
    assert_eq!(claim.identity(), &identity);
    for at in [12.0, 13.0, 14.0] {
        assert_eq!(
            broker.invoke_claimed_tool(&claim, at, |current| Ok(current.clone())),
            Ok(identity.clone())
        );
        assert_eq!(broker.bindings[&session.id].calls, 1);
    }
    assert_eq!(
        broker.claim_tool_call(&session, &operation(), 15.0).err(),
        Some(ErrorCode::BudgetExceeded)
    );
    assert_eq!(
        broker.invoke(&session, &operation(), 15.0, |_| panic!(
            "Fresh call at exhausted budget"
        )),
        Err::<(), _>(ErrorCode::BudgetExceeded)
    );
}
#[test]
fn revocation_identical_reapproval_or_closed_sessions_never_reactivate_a_claim() {
    for reason in ["revoke", "reapprove", "close"] {
        let (mut broker, identity, command, grant) = fixture("root", 1);
        broker.approve(identity.clone(), grant.clone()).unwrap();
        let session = broker.bind(identity.clone(), command, 100.0, 10.0).unwrap();
        let claim = broker
            .claim_tool_call(&session, &operation(), 11.0)
            .unwrap();
        if reason == "close" {
            broker.close(&session);
        } else {
            broker.revoke(&identity).unwrap();
            if reason == "reapprove" {
                broker.approve(identity, grant).unwrap();
            }
        }
        assert_eq!(
            broker.invoke_claimed_tool(&claim, 12.0, |_| panic!("Stale claim")),
            Err::<(), _>(if reason == "close" {
                ErrorCode::SessionInvalid
            } else {
                ErrorCode::GrantRevoked
            })
        );
    }
}
#[test]
fn expiry_deadline_and_nonfinite_clocks_refuse_a_previously_charged_call() {
    let (mut broker, identity, command, mut grant) = fixture("root", 1);
    grant.expires_at = 80.0;
    broker.approve(identity.clone(), grant).unwrap();
    let session = broker.bind(identity, command, 100.0, 10.0).unwrap();
    let claim = broker
        .claim_tool_call(&session, &operation(), 11.0)
        .unwrap();
    for (at, expected) in [
        (80.0, ErrorCode::GrantRevoked),
        (100.0, ErrorCode::Timeout),
        (f64::NAN, ErrorCode::Timeout),
    ] {
        assert_eq!(
            broker.invoke_claimed_tool(&claim, at, |_| panic!("Expired claim")),
            Err::<(), _>(expected)
        );
    }
}
#[test]
fn delegated_claims_keep_all_ancestor_scopes_epochs_and_monotonic_charges() {
    for reason in ["parent", "leaf", "closed-parent"] {
        let (mut broker, parent_id, command, grant) = fixture("root", 3);
        let (_, leaf_id, _, mut leaf_grant) = fixture("provider", 1);
        leaf_grant.expires_at = grant.expires_at;
        broker.approve(parent_id.clone(), grant).unwrap();
        broker.approve(leaf_id.clone(), leaf_grant).unwrap();
        let parent = broker
            .bind(parent_id.clone(), command.clone(), 100.0, 10.0)
            .unwrap();
        let leaf = broker
            .delegate(&parent, leaf_id.clone(), command, 90.0, 11.0)
            .unwrap();
        let claim = broker.claim_tool_call(&leaf, &operation(), 12.0).unwrap();
        assert_eq!(broker.bindings[&parent.id].calls, 2); // delegation plus this tool call
        assert_eq!(broker.bindings[&leaf.id].calls, 1);
        broker
            .invoke(&parent, &operation(), 13.0, |_| Ok(()))
            .unwrap();
        assert_eq!(broker.bindings[&parent.id].calls, 3);
        assert_eq!(
            broker.invoke_claimed_tool(&claim, 14.0, |current| Ok(current.clone())),
            Ok(leaf_id.clone())
        );
        match reason {
            "parent" => {
                broker.revoke(&parent_id).unwrap();
            }
            "leaf" => {
                broker.revoke(&leaf_id).unwrap();
            }
            _ => broker.close(&parent),
        }
        assert_eq!(
            broker.invoke_claimed_tool(&claim, 15.0, |_| panic!("Inactive ancestor/leaf")),
            Err::<(), _>(if reason == "closed-parent" {
                ErrorCode::SessionInvalid
            } else {
                ErrorCode::GrantRevoked
            })
        );
    }
}
#[test]
fn phase_entry_checks_current_scopes_and_the_complete_original_parent_chain() {
    for reason in ["manifest", "grant", "chain"] {
        let (mut broker, parent_id, command, grant) = fixture("root", 3);
        let (_, leaf_id, _, leaf_grant) = fixture("provider", 1);
        broker.approve(parent_id.clone(), grant).unwrap();
        broker.approve(leaf_id.clone(), leaf_grant).unwrap();
        let parent = broker
            .bind(parent_id, command.clone(), 100.0, 10.0)
            .unwrap();
        let leaf = broker
            .delegate(&parent, leaf_id.clone(), command, 90.0, 11.0)
            .unwrap();
        let claim = broker.claim_tool_call(&leaf, &operation(), 12.0).unwrap();
        // Fault injection without an epoch replacement verifies that each phase
        // really checks scope/chain, rather than trusting an earlier approval.
        match reason {
            "manifest" => {
                broker.bindings.get_mut(&parent.id).unwrap().command["permissions"][0]["scopes"] =
                    json!(["lock:ffmpeg:transcode-webm-v1"])
            }
            "grant" => broker
                .policies
                .get_mut(&leaf_id)
                .unwrap()
                .grant
                .as_mut()
                .unwrap()
                .scopes
                .clear(),
            _ => broker.bindings.get_mut(&leaf.id).unwrap().parents.clear(),
        }
        assert_eq!(
            broker.invoke_claimed_tool(&claim, 13.0, |_| panic!("Changed scope/chain")),
            Err::<(), _>(if reason == "chain" {
                ErrorCode::SessionInvalid
            } else {
                ErrorCode::ScopeDenied
            })
        );
    }
}
#[test]
fn non_tool_operations_do_not_consume_a_claim_and_another_broker_cannot_use_it() {
    let (mut broker, identity, command, grant) = fixture("root", 1);
    broker.approve(identity.clone(), grant.clone()).unwrap();
    let session = broker
        .bind(identity.clone(), command.clone(), 100.0, 10.0)
        .unwrap();
    assert_eq!(
        broker
            .claim_tool_call(&session, &CapabilityOperation::ClipboardRead, 11.0)
            .err(),
        Some(ErrorCode::CapabilityUndeclared)
    );
    assert_eq!(broker.bindings[&session.id].calls, 0);
    let claim = broker
        .claim_tool_call(&session, &operation(), 12.0)
        .unwrap();
    let mut other = CapabilityBroker::default();
    other.approve(identity.clone(), grant).unwrap();
    other.bind(identity, command, 100.0, 10.0).unwrap();
    assert_eq!(
        other.invoke_claimed_tool(&claim, 13.0, |_| panic!("Foreign broker")),
        Err::<(), _>(ErrorCode::SessionInvalid)
    );
}
