#[cfg(any(feature = "standalone", test))]
mod bundle;
#[cfg(windows)]
mod process_job;
#[cfg(windows)]
mod profile;
#[cfg(windows)]
mod security;
#[cfg(windows)]
mod server;
#[cfg(windows)]
mod startup;

fn main() {
    #[cfg(windows)]
    let result =
        startup::dispatch(&std::env::args().skip(1).collect::<Vec<_>>()).unwrap_or_else(|| {
            tokio::runtime::Builder::new_multi_thread()
                .worker_threads(2)
                .enable_all()
                .build()
                .map_err(|_| "STARTUP_FAILED")?
                .block_on(server::run())
        });
    #[cfg(not(windows))]
    let result: Result<(), &'static str> = Err("UNSUPPORTED_PLATFORM");
    if let Err(code) = result {
        eprintln!("{code}");
        std::process::exit(1);
    }
}
