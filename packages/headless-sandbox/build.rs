use sha2::{Digest, Sha256};
use std::{env, fs, path::Path};

fn main() {
    let vendor = Path::new("vendor/quickjs-ng");
    println!("cargo:rerun-if-changed=vendor/quickjs-ng/SHA256SUMS");
    for line in fs::read_to_string(vendor.join("SHA256SUMS"))
        .unwrap()
        .lines()
    {
        let (expected, name) = line.split_once("  ").expect("SHA256SUMS syntax");
        let path = vendor.join(name);
        println!("cargo:rerun-if-changed={}", path.display());
        let actual = format!("{:x}", Sha256::digest(fs::read(&path).unwrap()));
        assert_eq!(actual, expected, "pinned engine drift: {name}");
    }
    println!("cargo:rerun-if-changed=src/engine.c");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows")
        || env::var("CARGO_CFG_TARGET_ARCH").as_deref() != Ok("x86_64")
    {
        return;
    }
    assert!(
        env::var("CARGO_CFG_TARGET_FEATURE")
            .unwrap()
            .split(',')
            .any(|feature| feature == "crt-static"),
        "run Cargo from packages/headless-sandbox so its fixed static CRT configuration is loaded"
    );
    cc::Build::new()
        .static_crt(true)
        .include(vendor)
        .files([
            "vendor/quickjs-ng/quickjs.c",
            "vendor/quickjs-ng/dtoa.c",
            "vendor/quickjs-ng/libregexp.c",
            "vendor/quickjs-ng/libunicode.c",
            "src/engine.c",
        ])
        .define("QUICKJS_NG_BUILD", None)
        .define("_GNU_SOURCE", None)
        .define("WIN32_LEAN_AND_MEAN", None)
        .define("_WIN32_WINNT", "0x0A00")
        .define("__STDC_NO_ATOMICS__", "1")
        .define("NDEBUG", None)
        .flag_if_supported("/std:c11")
        .flag_if_supported("/utf-8")
        .warnings(false)
        .opt_level(2)
        .compile("flowtools_p22_quickjs_engine");
    println!("cargo:rustc-link-arg-bin=flowtools-p22-sandbox-child=/STACK:8388608");
    println!("cargo:rustc-link-arg-bin=flowtools-p22-sandbox-child=/DELAYLOAD:WS2_32.dll");
    println!("cargo:rustc-link-arg-bin=flowtools-p22-sandbox-child=delayimp.lib");
    let out = std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let child = out
        .ancestors()
        .nth(3)
        .unwrap()
        .join("flowtools-p22-sandbox-child.exe");
    println!(
        "cargo:rustc-env=FLOWTOOLS_P22_CHILD_BINARY={}",
        child.display()
    );
    let probe = out.join("flowtools-p22-startup-probe.exe");
    let tool = cc::Build::new().static_crt(true).get_compiler();
    let status = tool
        .to_command()
        .arg("/nologo")
        .arg("/O1")
        .arg("/GS-")
        .arg("/Zl")
        .arg("src/startup-probe.c")
        .arg(format!("/Fo{}", out.join("startup-probe.obj").display()))
        .arg(format!("/Fe{}", probe.display()))
        .arg("/link")
        .arg("/NODEFAULTLIB")
        .arg("/ENTRY:main")
        .arg("/SUBSYSTEM:CONSOLE")
        .arg("kernel32.lib")
        .status()
        .unwrap();
    assert!(status.success(), "compile fixed native startup control");
    println!(
        "cargo:rustc-env=FLOWTOOLS_P22_STARTUP_PROBE={}",
        probe.display()
    );
    println!("cargo:rerun-if-changed=src/startup-probe.c");
}
