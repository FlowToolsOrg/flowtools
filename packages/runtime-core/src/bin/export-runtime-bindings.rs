fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg != "--check") || args.len() > 1 {
        return Err("Expected only --check".into());
    }
    flowtools_runtime_core::codegen::write_or_check(!args.is_empty())
}
