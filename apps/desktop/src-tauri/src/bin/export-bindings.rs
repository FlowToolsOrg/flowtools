fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../src/utils/bindings.ts");
    desktop_lib::export_bindings(&path)?;
    println!("Exported desktop bindings to {}", path.display());
    Ok(())
}
