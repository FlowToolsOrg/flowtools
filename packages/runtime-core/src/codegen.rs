use crate::{
    broker::{CapabilityOperation, ReadMethod, SendMethod},
    catalog::{digest, BuiltinCatalog},
    protocol::*,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn manifest_fixture(manifest: &Value) -> Value {
    let mut fixture = manifest.clone();
    // Contract goldens use controlled LF text, never checkout-dependent sourcemaps.
    // Runtime/runner integrity still uses the unchanged build-owned catalog bytes.
    let mut paths: Vec<&str> = manifest["entries"]
        .as_object()
        .expect("Validated entries")
        .values()
        .map(|entry| entry.as_str().expect("Validated entry path"))
        .collect();
    paths.sort_unstable();
    paths.dedup();
    fixture["files"] = json!(paths
        .into_iter()
        .map(|path| {
            let text = format!("// Controlled G2 contract fixture: {path}\n");
            json!({"path":path,"sha256":format!("{:x}", Sha256::digest(text.as_bytes())),"size":text.len()})
        })
        .collect::<Vec<Value>>());
    fixture
}

pub fn artifacts() -> Result<Vec<(&'static str, String)>, Box<dyn std::error::Error>> {
    let types = specta::Types::default()
        .register::<CapabilityOperation>()
        .register::<Request>()
        .register::<Response>()
        .register::<StorageAction>()
        .register::<StorageReport>();
    let bindings = format!("// Generated from Rust. Do not edit.\nexport const PROTOCOL_MAJOR = {PROTOCOL_MAJOR} as const\nexport const CLIENT_VERSION = '{CLIENT_VERSION}' as const\n{}", specta_typescript::Typescript::default().export(&types, specta_serde::Format)?);
    let schemas = json!({ "request": schemars::schema_for!(Request), "response": schemars::schema_for!(Response), "capabilityOperation": schemars::schema_for!(CapabilityOperation), "storageAction": schemars::schema_for!(StorageAction), "storageReport": schemars::schema_for!(StorageReport) });
    let request = Request {
        version: PROTOCOL_MAJOR,
        request_id: "fixture-request".into(),
        session: None,
        call: Call::Submit(SubmitJob {
            plugin_id: "plugin-base64-encoder".into(),
            command_id: "run".into(),
            input: json!({"text":"hello"}),
            idempotency_key: "fixture-job".into(),
            background: true,
            deadline: 10000.0,
        }),
    };
    let errors: Vec<Response> = ErrorCode::ALL
        .into_iter()
        .map(|code| Response {
            version: PROTOCOL_MAJOR,
            request_id: "fixture-request".into(),
            outcome: Outcome::Error(RuntimeError { code }),
        })
        .collect();
    let manifests: Vec<Value> = BuiltinCatalog::embedded()
        .list()
        .map(|(_, manifest)| {
            let fixture = manifest_fixture(manifest);
            json!({"digest":digest(&fixture),"manifest":fixture})
        })
        .collect();
    let operations = vec![
        CapabilityOperation::FileRead {
            handle: "host-handle".into(),
        },
        CapabilityOperation::FileCreate {
            handle: "host-handle".into(),
        },
        CapabilityOperation::FileReplace {
            handle: "host-handle".into(),
        },
        CapabilityOperation::FileDelete {
            handle: "host-handle".into(),
        },
        CapabilityOperation::NetworkRead {
            origin: "https://example.invalid:443".into(),
            method: ReadMethod::HEAD,
        },
        CapabilityOperation::NetworkSend {
            origin: "https://example.invalid:443".into(),
            method: SendMethod::POST,
        },
        CapabilityOperation::DataRead {
            key: "todos".into(),
        },
        CapabilityOperation::DataWrite {
            key: "todos".into(),
        },
        CapabilityOperation::ClipboardRead,
        CapabilityOperation::ClipboardWrite,
        CapabilityOperation::ToolExecute {
            lock: "host-lock".into(),
            action: "probe".into(),
        },
    ];
    let fixtures = json!({"formatVersion":1,"request":request,"responses":errors,"manifests":manifests,"operations":operations});
    Ok(vec![
        ("bindings.ts", bindings),
        (
            "wire-schema.json",
            serde_json::to_string_pretty(&schemas)? + "\n",
        ),
        (
            "wire-fixtures.json",
            serde_json::to_string_pretty(&fixtures)? + "\n",
        ),
    ])
}

pub fn write_or_check(check: bool) -> Result<(), Box<dyn std::error::Error>> {
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime-client/src");
    for (name, output) in artifacts()? {
        let path = directory.join(name);
        if check {
            if std::fs::read_to_string(path)?.replace("\r\n", "\n") != output {
                return Err(format!("Generated Runtime artifact drift: {name}").into());
            }
        } else {
            std::fs::write(path, output)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_wire_and_complete_manifest_fixtures_roundtrip() {
        let artifacts = artifacts().unwrap();
        let schemas: Value = serde_json::from_str(&artifacts[1].1).unwrap();
        let fixtures: Value = serde_json::from_str(&artifacts[2].1).unwrap();
        let request: Request = serde_json::from_value(fixtures["request"].clone()).unwrap();
        assert!(jsonschema::is_valid(
            &schemas["request"],
            &serde_json::to_value(request).unwrap()
        ));
        let errors = fixtures["responses"].as_array().unwrap();
        assert_eq!(errors.len(), ErrorCode::ALL.len());
        for value in errors {
            let response: Response = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(&serde_json::to_value(response).unwrap(), value);
            assert!(jsonschema::is_valid(&schemas["response"], value));
        }
        let catalog = BuiltinCatalog::embedded();
        assert_eq!(fixtures["operations"].as_array().unwrap().len(), 11);
        for value in fixtures["operations"].as_array().unwrap() {
            let operation: CapabilityOperation = serde_json::from_value(value.clone()).unwrap();
            assert_eq!(&serde_json::to_value(operation).unwrap(), value);
            assert!(jsonschema::is_valid(&schemas["capabilityOperation"], value));
        }
        assert_eq!(fixtures["manifests"].as_array().unwrap().len(), 12);
        for fixture in fixtures["manifests"].as_array().unwrap() {
            let (built, _) = catalog
                .command(fixture["manifest"]["id"].as_str().unwrap(), "run")
                .unwrap();
            let mut restored = fixture["manifest"].clone();
            restored["files"] = built["files"].clone();
            catalog.validate_manifest(&restored).unwrap();
            assert_eq!(fixture["manifest"], manifest_fixture(built));
            assert_eq!(fixture["digest"], digest(&fixture["manifest"]));
            let mut different_artifact = built.clone();
            different_artifact["files"][0]["sha256"] = json!("0".repeat(64));
            assert!(catalog.validate_manifest(&different_artifact).is_err());
        }
        assert_eq!(artifacts, super::artifacts().unwrap());
        write_or_check(true).unwrap();
    }
}
