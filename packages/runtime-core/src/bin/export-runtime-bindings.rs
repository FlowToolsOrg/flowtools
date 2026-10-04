use flowtools_runtime_core::protocol::{Request, Response};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let types = specta::Types::default()
        .register::<Request>()
        .register::<Response>();
    let output = specta_typescript::Typescript::default().export(&types, specta_serde::Format)?;
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../runtime-client/src/bindings.ts");
    std::fs::write(path, output)?;
    let schemas = serde_json::json!({
        "request": schemars::schema_for!(Request),
        "response": schemars::schema_for!(Response),
    });
    std::fs::write(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../runtime-client/src/wire-schema.json"),
        serde_json::to_string_pretty(&schemas)? + "\n",
    )?;
    Ok(())
}
