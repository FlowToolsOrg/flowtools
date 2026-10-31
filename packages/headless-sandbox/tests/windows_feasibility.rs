#![cfg(all(windows, target_arch = "x86_64"))]

use flowtools_headless_sandbox::{
    Fixture, Outcome, Rejection, INPUT_LIMIT, TERMINATE_GRACE_MS, WALL_LIMIT_MS,
};
use serde_json::{json, Value};
use std::{
    fs,
    net::{TcpListener, TcpStream, UdpSocket},
    path::Path,
    time::Duration,
};

fn fixture() -> Fixture {
    Fixture::new().expect("LPAC setup must succeed; unsupported controls never fall back")
}

fn drained(outcome: &Outcome) {
    assert!(outcome.token.app_container);
    assert!(outcome.token.aap_access_denied);
    assert!(outcome.token.sid_matches);
    assert_eq!(outcome.token.capability_count, 0);
    assert_eq!(outcome.active_processes, 0, "{outcome:?}");
    assert!(
        outcome.elapsed < Duration::from_millis(WALL_LIMIT_MS + TERMINATE_GRACE_MS + 100),
        "{outcome:?}"
    );
}

fn value(outcome: Outcome) -> Value {
    drained(&outcome);
    assert_eq!(outcome.rejection, None, "{outcome:?}");
    assert_eq!(outcome.exit_code, 0);
    outcome.value.expect("bounded JSON result")
}

fn network_refused(result: &Value, final_stage: &str) {
    eprintln!("Native {final_stage} refusal: {result}");
    assert_eq!(result["allowed"], false, "{result}");
    assert!(
        (result["osError"] == 10107 && result["stage"] == "WSAStartup")
            || (result["osError"] == 10013
                && (result["stage"] == "socket" || result["stage"] == final_stage)),
        "Only a stage-specific native refusal counts: {result}"
    );
}

#[test]
fn actual_lpac_executes_bounded_pure_json_without_native_js_apis() {
    let fixture = fixture();
    assert_eq!(value(fixture.execute("return {sum: input.left + input.right, api: [typeof process, typeof require, typeof std, typeof os, typeof fetch, typeof WebSocket, typeof Worker]}", json!({"left": 20, "right": 22})).unwrap()),
        json!({"value":{"sum":42,"api":["undefined","undefined","undefined","undefined","undefined","undefined","undefined"]}}));
    assert_eq!(
        value(
            fixture
                .execute(
                    "return input",
                    json!({"text":"\u{2028} literal \" quote","array":[null,false,1.25]})
                )
                .unwrap()
        ),
        json!({"value":{"text":"\u{2028} literal \" quote","array":[null,false,1.25]}})
    );
}

#[test]
fn actual_native_file_reads_and_writes_refuse_private_t0_bytes() {
    let fixture = fixture();
    let original = fs::read(fixture.canary_path()).unwrap();
    for operation in ["read", "write"] {
        let result = value(
            fixture
                .native_probe(json!({"operation":operation,"path":fixture.canary_path()}))
                .unwrap(),
        );
        assert_eq!(result["allowed"], false, "{result}");
        assert_eq!(result["osError"], 5, "{result}");
    }
    // Positive native control: the explicit SID image grant really works.
    assert_eq!(
        value(
            fixture
                .native_probe(json!({"operation":"read","path":fixture.executable_path()}))
                .unwrap()
        )["allowed"],
        true
    );
    assert_eq!(
        value(
            fixture
                .native_probe(json!({"operation":"write","path":fixture.executable_path()}))
                .unwrap()
        )["allowed"],
        false
    );
    assert_eq!(fs::read(fixture.canary_path()).unwrap(), original);
}

#[test]
fn actual_native_tcp_and_udp_are_denied_without_network_capabilities() {
    let fixture = fixture();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    // Parent connectivity proves this is a live socket, not an absent endpoint.
    let control = TcpStream::connect(address).unwrap();
    let (_accepted, _) = listener.accept().unwrap();
    drop(control);
    listener.set_nonblocking(true).unwrap();
    let result = value(
        fixture
            .native_probe(json!({"operation":"network","address":address.to_string()}))
            .unwrap(),
    );
    network_refused(&result, "connect");
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
    let receiver = UdpSocket::bind("127.0.0.1:0").unwrap();
    let address = receiver.local_addr().unwrap();
    let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    assert_eq!(sender.send_to(b"host-control", address).unwrap(), 12);
    let mut buffer = [0_u8; 64];
    let (count, peer) = receiver.recv_from(&mut buffer).unwrap();
    assert_eq!(&buffer[..count], b"host-control");
    assert_eq!(peer, sender.local_addr().unwrap());
    receiver.set_read_timeout(None).unwrap();
    receiver.set_nonblocking(true).unwrap();
    let result = value(
        fixture
            .native_probe(json!({"operation":"udp","address":address.to_string()}))
            .unwrap(),
    );
    network_refused(&result, "sendto");
    assert_eq!(
        receiver.recv_from(&mut buffer).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[test]
fn native_process_creation_is_denied_and_owned_job_is_empty() {
    let fixture = fixture();
    let result = value(fixture.native_probe(json!({"operation":"child"})).unwrap());
    assert_eq!(result["allowed"], false, "{result}");
    assert!(
        result["osError"]
            .as_i64()
            .is_some_and(|code| [5, 1260, 367].contains(&code)),
        "{result}"
    );
}

#[test]
fn parent_environment_and_nonwhitelisted_inheritable_handle_do_not_cross() {
    let fixture = fixture();
    // The host has PATH; the child receives two OS-derived bootstrap values.
    // Windows rewrites LOCALAPPDATA and adds its AppContainer TEMP/TMP paths.
    assert!(std::env::var_os("PATH").is_some());
    let result = value(
        fixture
            .native_probe(json!({"operation":"environment"}))
            .unwrap(),
    );
    let variables = result["variables"].as_array().unwrap();
    assert_eq!(
        variables
            .iter()
            .map(|entry| entry[0].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["LOCALAPPDATA", "SystemRoot", "TEMP", "TMP"]
    );
    assert_eq!(
        Path::new(variables[0][1].as_str().unwrap()),
        fixture.os_profile_path()
    );
    assert_eq!(
        Path::new(variables[2][1].as_str().unwrap()),
        fixture.os_profile_path().join("Temp")
    );
    assert_eq!(variables[2][1], variables[3][1]);
    assert!(!result.to_string().contains("PATH"));
    let included = value(fixture.probe_included_handle().unwrap());
    assert_eq!(included["parentCanarySignaled"], true, "{included}");
    assert_eq!(included["probe"]["setEventReturned"], true, "{included}");
    let outcome = fixture.probe_unlisted_handle().unwrap();
    drained(&outcome);
    let result = outcome.value.as_ref().unwrap();
    eprintln!("Unlisted Event result: {outcome:?}");
    assert_eq!(result["parentCanarySignaled"], false, "{outcome:?}");
    match outcome.rejection {
        None => {
            assert_eq!(outcome.exit_code, 0, "{outcome:?}");
            assert_eq!(result["probe"]["setEventReturned"], false, "{outcome:?}");
            assert_eq!(result["probe"]["osError"], 6, "{outcome:?}");
        }
        Some(Rejection::ProcessFailure) => {
            assert_eq!(outcome.exit_code, 0xc0000008, "{outcome:?}");
            assert!(result["probe"].is_null(), "{outcome:?}");
        }
        other => panic!("Unlisted Event refusal must not time out or accept arbitrary failure: {other:?}; {outcome:?}"),
    }
    fixture.close().unwrap();
}

#[test]
fn inherited_error_policy_and_native_crash_refuse_without_blocking_ui() {
    use windows_sys::Win32::System::{
        Diagnostics::Debug::{
            SEM_FAILCRITICALERRORS, SEM_NOGPFAULTERRORBOX, SEM_NOOPENFILEERRORBOX,
        },
        ErrorReporting::WER_FAULT_REPORTING_NO_UI,
    };
    let fixture = fixture();
    let policy = value(
        fixture
            .native_probe(json!({"operation":"error-ui"}))
            .unwrap(),
    );
    eprintln!("Child error UI policy: {policy}");
    let required = SEM_FAILCRITICALERRORS | SEM_NOGPFAULTERRORBOX | SEM_NOOPENFILEERRORBOX;
    assert_eq!(
        policy["inheritedErrorMode"].as_u64().unwrap() as u32 & required,
        required,
        "{policy}"
    );
    assert_eq!(
        policy["errorMode"].as_u64().unwrap() as u32 & required,
        required,
        "{policy}"
    );
    assert_eq!(policy["werResult"], 0, "{policy}");
    assert_eq!(policy["werFlags"], WER_FAULT_REPORTING_NO_UI, "{policy}");
    let crash = fixture.native_probe(json!({"operation":"crash"})).unwrap();
    drained(&crash);
    assert_eq!(
        crash.rejection,
        Some(Rejection::ProcessFailure),
        "{crash:?}"
    );
    assert_eq!(crash.exit_code, 0xe0422201, "{crash:?}");
    assert!(crash.value.is_none());
    assert_eq!(
        value(fixture.execute("return 42", Value::Null).unwrap()),
        json!({"value":42})
    );
    fixture.close().unwrap();
}

#[test]
fn engine_loops_memory_stack_and_output_fail_without_host_failure() {
    let fixture = fixture();
    for (source, expected) in [
        ("while (true) {}", "DEADLINE"),
        (
            "const a=[]; while(true) a.push(new Array(100000).fill(42))",
            "ENGINE_REJECTED",
        ),
        ("function f(){return f()} return f()", "ENGINE_REJECTED"),
        ("return 'x'.repeat(20000)", "OUTPUT_LIMIT"),
        (
            "throw new Error('PRIVATE_EXCEPTION_CANARY')",
            "ENGINE_REJECTED",
        ),
    ] {
        assert_eq!(
            value(fixture.execute(source, Value::Null).unwrap()),
            json!({"error":expected})
        );
        assert_eq!(
            value(fixture.execute("return 42", Value::Null).unwrap()),
            json!({"value":42})
        );
    }
}

#[test]
fn parent_kills_native_hangs_and_output_floods() {
    let fixture = fixture();
    for (operation, expected) in [
        ("hang", Rejection::Deadline),
        ("output", Rejection::OutputLimit),
    ] {
        let outcome = fixture
            .native_probe(json!({"operation":operation}))
            .unwrap();
        drained(&outcome);
        assert_eq!(outcome.rejection, Some(expected), "{outcome:?}");
        assert!(outcome.value.is_none());
        assert_eq!(
            value(fixture.execute("return 42", Value::Null).unwrap()),
            json!({"value":42})
        );
    }
}

#[test]
fn oversized_input_is_rejected_before_spawn_and_fixture_cleanup_is_owned() {
    let fixture = fixture();
    let root = fixture.root_path().to_path_buf();
    let error = fixture
        .execute(&"x".repeat(INPUT_LIMIT), Value::Null)
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    assert_eq!(
        fs::read(fixture.canary_path()).unwrap(),
        b"T0_PRIVATE_CANARY"
    );
    drop(fixture);
    assert!(
        !root.exists(),
        "fixture cleanup must complete after Job drain"
    );
}

#[test]
fn actual_job_memory_budget_refuses_large_commit_after_small_control() {
    let fixture = fixture();
    let result = value(fixture.native_probe(json!({"operation":"memory"})).unwrap());
    assert_eq!(result["control"], true, "{result}");
    assert_eq!(result["allowed"], false, "{result}");
    assert!(
        result["osError"]
            .as_i64()
            .is_some_and(|code| [8, 1455].contains(&code)),
        "{result}"
    );
}

#[test]
fn engine_input_has_owned_nul_terminator_and_rejects_embedded_nul() {
    let fixture = fixture();
    assert_eq!(
        value(
            fixture
                .execute("return 42\0; throw 'not executed'", Value::Null)
                .unwrap()
        ),
        json!({"error":"INPUT_INVALID"})
    );
    let source = format!("{} return input", " ".repeat(INPUT_LIMIT - 200));
    assert_eq!(
        value(fixture.execute(&source, json!("edge")).unwrap()),
        json!({"value":"edge"})
    );
}

#[test]
fn sibling_sid_cannot_read_another_fixture_image_or_private_canary() {
    let first = fixture();
    let second = fixture();
    for path in [
        first.executable_path(),
        first.canary_path(),
        first.aap_canary_path(),
    ] {
        let result = value(
            second
                .native_probe(json!({"operation":"read","path":path}))
                .unwrap(),
        );
        assert_eq!(result["allowed"], false, "{result}");
        assert_eq!(result["osError"], 5, "{result}");
    }
    let own = value(
        second
            .native_probe(json!({"operation":"read","path":second.executable_path()}))
            .unwrap(),
    );
    assert_eq!(own["allowed"], true);
    first.close().unwrap();
    assert_eq!(
        value(second.execute("return 42", Value::Null).unwrap()),
        json!({"value":42})
    );
    second.close().unwrap();
}

#[test]
fn json_own_keys_preserve_proto_constructor_and_nested_objects() {
    let fixture = fixture();
    let input = json!({"__proto__":{"injected":true},"constructor":{"prototype":{"value":1}},"nested":{"__proto__":null}});
    let result = value(fixture.execute("return {own:Object.hasOwn(input,'__proto__'), inherited:input.injected === undefined, constructorOwn:Object.hasOwn(input,'constructor'), nestedOwn:Object.hasOwn(input.nested,'__proto__'), original:input}", input.clone()).unwrap());
    assert_eq!(
        result,
        json!({"value":{"own":true,"inherited":true,"constructorOwn":true,"nestedOwn":true,"original":input}})
    );
}
