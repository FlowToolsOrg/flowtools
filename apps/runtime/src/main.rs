#[cfg(windows)]
mod profile;
#[cfg(windows)]
mod security;
#[cfg(windows)]
mod server;

#[tokio::main]
async fn main() {
    #[cfg(windows)]
    let result = server::run().await;
    #[cfg(not(windows))]
    let result: Result<(), &'static str> = Err("UNSUPPORTED_PLATFORM");
    if let Err(code) = result {
        eprintln!("{code}");
        std::process::exit(1);
    }
}
