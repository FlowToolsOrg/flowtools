//! Opt-in, disposable native feasibility harness. Never launches desktop_lib.
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tauri::{http, webview::NewWindowResponse, Manager, WebviewUrl, WebviewWindowBuilder};

const ID: &str = "com.flowtools.g5-plugin-webviews-validation-20261012";
const TITLE: &str = "FlowTools G5 WebView validation";
const HOST: &str = "https://g5host.localhost/index.html";
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self' ipc: http://ipc.localhost https://ipc.localhost; img-src 'self'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";
const HTML: &str = include_str!("../../../../ui-test/fixtures/plugin-webviews/index.html");
const SCRIPT: &str = include_str!("../../../../ui-test/fixtures/plugin-webviews/fixture.js");

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Session {
    source: String,
    session: String,
    nonce: String,
    generation: u32,
    sequence: u32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Probe {
    source: String,
    session: String,
    nonce: String,
    generation: u32,
    sequence: u32,
    operation: String,
}

#[derive(Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Evidence {
    accepted: u32,
    rejected: u32,
    invoke_denied: u32,
    navigation_denied: u32,
    popup_denied: u32,
    download_denied: u32,
    profiles_configured: u32,
    sessions_revoked: u32,
    documents_served: u32,
    external_requests_denied: u32,
    cross_origin_resources_denied: u32,
    #[serde(skip)]
    sessions: BTreeMap<String, Session>,
}

fn expected(label: &str) -> Option<&'static str> {
    match label {
        "g5-host" => Some(HOST),
        "g5-a" | "g5-a-partition" => Some("https://g5a.localhost/index.html"),
        "g5-b" => Some("https://g5b.localhost/index.html"),
        _ => None,
    }
}

#[cfg(windows)]
fn nonce() -> String {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes).expect("OS-generated validation nonce");
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(not(windows))]
fn nonce() -> String {
    panic!("Windows-only native feasibility harness")
}

#[cfg(windows)]
fn configure_native_policy(
    window: &tauri::WebviewWindow,
    directory: PathBuf,
    destination: &'static str,
    evidence: Arc<Mutex<Evidence>>,
) -> tauri::Result<()> {
    use webview2_com::{
        Microsoft::Web::WebView2::Win32::{
            ICoreWebView2_13, ICoreWebView2_22, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
        },
        WebResourceRequestedEventHandler,
    };
    use windows_core::{Interface, HSTRING, PWSTR};
    std::fs::create_dir(&directory).expect("Create private validation download directory");
    window.with_webview(move |webview| {
        // COM access remains on Tauri's WebView thread and uses generated ABI.
        let core = unsafe { webview.controller().CoreWebView2().expect("Core WebView") };
        let profile = unsafe {
            core.cast::<ICoreWebView2_13>()
                .expect("WebView2 profile API")
                .Profile()
                .expect("Native profile")
        };
        let mut actual = PWSTR::null();
        unsafe {
            profile
                .SetDefaultDownloadFolderPath(&HSTRING::from(directory.as_os_str()))
                .expect("Set private native download directory");
            profile
                .DefaultDownloadFolderPath(&mut actual)
                .expect("Read native download directory");
        }
        assert_eq!(PathBuf::from(webview2_com::take_pwstr(actual)), directory);
        evidence.lock().expect("Evidence lock").profiles_configured += 1;
        let environment = webview.environment();
        let requests = evidence.clone();
        let mut token = 0;
        unsafe {
            core.cast::<ICoreWebView2_22>()
                .expect("WebView2 all-source request filter")
                .AddWebResourceRequestedFilterWithRequestSourceKinds(
                    &HSTRING::from("*"),
                    COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
                    COREWEBVIEW2_WEB_RESOURCE_REQUEST_SOURCE_KINDS_ALL,
                )
                .expect("Install all-source request filter");
            core.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_, args| {
                    let args = args.expect("Native request event");
                    let mut uri = PWSTR::null();
                    args.Request()?.Uri(&mut uri)?;
                    let uri = webview2_com::take_pwstr(uri);
                    let allowed = tauri::Url::parse(&uri).is_ok_and(|url| {
                        let own = tauri::Url::parse(destination).expect("Fixed destination");
                        (url.scheme() == "https" && url.origin() == own.origin())
                            || (matches!(url.scheme(), "http" | "https")
                                && url.host_str() == Some("ipc.localhost")
                                && url.port().is_none())
                    });
                    if !allowed {
                        requests
                            .lock()
                            .expect("Evidence lock")
                            .external_requests_denied += 1;
                        let denied = environment.CreateWebResourceResponse(
                            None,
                            403,
                            &HSTRING::from("Validation request denied"),
                            &HSTRING::from(
                                "Content-Type: text/plain\r\nCache-Control: no-store\r\n",
                            ),
                        )?;
                        args.SetResponse(&denied)?;
                    }
                    Ok(())
                })),
                &mut token,
            )
            .expect("Install native request denial");
            // Policy precedes the first fixture document, not merely later effects.
            core.Navigate(&HSTRING::from(destination))
                .expect("Navigate after policy");
        }
    })
}

fn fixture_root() -> Result<(PathBuf, [u16; 4]), String> {
    if !cfg!(all(debug_assertions, target_os = "windows")) {
        return Err("Windows Debug validation only".into());
    }
    if std::env::var_os("WEBVIEW2_USER_DATA_FOLDER").is_some()
        || std::env::var_os("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS").is_some()
    {
        return Err("Global WebView profile/browser overrides are forbidden".into());
    }
    let root = std::env::var_os("FLOWTOOLS_PLUGIN_WEBVIEW_VALIDATION_ROOT")
        .map(PathBuf::from)
        .ok_or("Disposable validation root required")?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let instance = root
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("Validation instance name required")?;
    if std::env::args().skip(1).collect::<Vec<_>>() != [format!("--validation-instance={instance}")]
    {
        return Err("Exact disposable validation instance argument required".into());
    }
    let temp = std::env::temp_dir()
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if root.parent() != Some(temp.as_path())
        || !root.file_name().is_some_and(|name| {
            name.to_string_lossy()
                .starts_with("flowtools-plugin-webview-")
        })
        || std::fs::read_dir(&root)
            .map_err(|e| e.to_string())?
            .next()
            .is_some()
    {
        return Err("Require a fresh direct child of the temporary directory".into());
    }
    let ports: Vec<u16> = std::env::var("FLOWTOOLS_PLUGIN_WEBVIEW_VALIDATION_CDP_PORTS")
        .map_err(|_| "Child-only loopback CDP ports required")?
        .split(',')
        .map(|value| value.parse::<u16>().map_err(|_| "Invalid CDP port"))
        .collect::<Result<_, _>>()?;
    if ports.len() != 4
        || ports.contains(&0)
        || ports
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            .len()
            != 4
    {
        return Err("Four distinct CDP ports required".into());
    }
    Ok((root, ports.try_into().expect("Four ports")))
}

fn response(status: u16, body: Vec<u8>, content_type: &str) -> http::Response<Vec<u8>> {
    http::Response::builder()
        .status(status)
        .header("Content-Type", content_type)
        .header("Content-Security-Policy", CSP)
        .header("Cache-Control", "no-store")
        .header("X-Content-Type-Options", "nosniff")
        .body(body)
        .expect("Fixed response")
}

fn main() {
    // The macro accepts only canonical tauri.conf filenames. Its capabilities
    // argument is additive; explicitly discard the generated production ACL.
    let mut context = tauri::generate_context!(capabilities = []);
    assert!(
        context
            .runtime_authority_mut()
            .resolve_access(
                "plugin:event|listen",
                "main",
                "main",
                &tauri::ipc::Origin::Local,
            )
            .is_some(),
        "Production ACL positive control"
    );
    *context.runtime_authority_mut() =
        tauri::runtime_authority!(Default::default(), Default::default());
    for label in ["main", "g5-host", "g5-a", "g5-b", "g5-a-partition"] {
        for command in [
            "plugin:event|listen",
            "plugin:window|set_title",
            "plugin:webview|create_webview",
        ] {
            assert!(
                context
                    .runtime_authority_mut()
                    .resolve_access(command, label, label, &tauri::ipc::Origin::Local,)
                    .is_none(),
                "Validation ACL must be empty for every label"
            );
        }
    }
    *context.config_mut() = serde_json::from_str(include_str!(
        "../../../tauri.plugin-webview-validation.conf.json"
    ))
    .expect("Embedded validation config");
    if std::env::args().nth(1).as_deref() == Some("--validation-identity") {
        println!(
            "{}",
            serde_json::json!({"identifier":context.config().identifier,"title":TITLE,"mode":"g5-webview-feasibility-v1","nativeAclCleared":true,"productionAclControl":true})
        );
        return;
    }
    assert_eq!(context.config().identifier, ID);
    let (root, ports) = fixture_root().expect("Dedicated validation environment");
    let evidence = Arc::new(Mutex::new(Evidence::default()));
    let mut builder = tauri::Builder::default();
    for protocol in ["g5host", "g5a", "g5b"] {
        let evidence = evidence.clone();
        builder = builder.register_uri_scheme_protocol(protocol, move |ctx, request| {
            let own_protocol = expected(ctx.webview_label()).is_some_and(|url| {
                tauri::Url::parse(url)
                    .is_ok_and(|url| url.host_str() == Some(&format!("{protocol}.localhost")))
            });
            if !own_protocol {
                evidence
                    .lock()
                    .expect("Evidence lock")
                    .cross_origin_resources_denied += 1;
                return response(403, Vec::new(), "text/plain");
            }
            match (request.method().as_str(), request.uri().path()) {
                ("GET", "/index.html") => {
                    evidence.lock().expect("Evidence lock").documents_served += 1;
                    response(200, HTML.as_bytes().to_vec(), "text/html; charset=utf-8")
                }
                ("GET", "/fixture.js") => response(
                    200,
                    SCRIPT.as_bytes().to_vec(),
                    "text/javascript; charset=utf-8",
                ),
                ("GET", "/download") => {
                    let mut result = response(
                        200,
                        b"inert validation download".to_vec(),
                        "application/octet-stream",
                    );
                    result.headers_mut().insert(
                        "Content-Disposition",
                        "attachment; filename=validation-denied.txt"
                            .parse()
                            .expect("Header"),
                    );
                    result
                }
                ("POST", "/probe") => {
                    let label = ctx.webview_label();
                    let actual = ctx
                        .app_handle()
                        .get_webview_window(label)
                        .and_then(|w| w.url().ok());
                    let mut state = evidence.lock().expect("Evidence lock");
                    let probe = if request.body().len() <= 2048 {
                        serde_json::from_slice::<Probe>(request.body()).ok()
                    } else {
                        None
                    };
                    let valid = probe.as_ref().is_some_and(|p| {
                        state.sessions.get(label).is_some_and(|s| {
                            expected(label).is_some_and(|url| {
                                actual.as_ref().is_some_and(|a| a.as_str() == url)
                            }) && actual.as_ref().is_some_and(|url| {
                                url.host_str() == Some(&format!("{protocol}.localhost"))
                                    && request
                                        .headers()
                                        .get(http::header::ORIGIN)
                                        .and_then(|h| h.to_str().ok())
                                        == Some(url.origin().ascii_serialization().as_str())
                            }) && p.source == label
                                && p.source == s.source
                                && p.session == s.session
                                && p.nonce == s.nonce
                                && p.generation == s.generation
                                && p.sequence == s.sequence
                                && p.operation == "echo"
                                && s.sequence < 16
                        })
                    });
                    if valid {
                        state.accepted += 1;
                        state
                            .sessions
                            .get_mut(label)
                            .expect("Validated session")
                            .sequence += 1;
                    } else {
                        state.rejected += 1;
                    }
                    response(
                        if valid { 200 } else { 403 },
                        br#"{"ok":true}"#.to_vec(),
                        "application/json",
                    )
                }
                _ => response(404, Vec::new(), "text/plain"),
            }
        });
    }
    let calls = evidence.clone();
    builder
        .invoke_handler(move |invoke| {
            let webview = invoke.message.webview_ref();
            let host = webview.label() == "g5-host" && webview.url().is_ok_and(|u| u.as_str() == HOST);
            let empty_args = matches!(invoke.message.payload(), tauri::ipc::InvokeBody::Json(value) if value.as_object().is_some_and(|object| object.is_empty()));
            if host && empty_args && invoke.message.command() == "validation_snapshot" {
                invoke.resolver.resolve(serde_json::to_value(&*calls.lock().expect("Evidence lock")).expect("Evidence JSON"));
            } else {
                calls.lock().expect("Evidence lock").invoke_denied += 1;
                invoke.resolver.reject("VALIDATION_NATIVE_DENIED");
            }
            true
        })
        .setup(move |app| {
            for (index, (label, directory)) in [
                ("g5-host", "host"),
                ("g5-a", "a"),
                ("g5-b", "b"),
                ("g5-a-partition", "a-partition"),
            ].into_iter().enumerate() {
                let session = Session {
                    source: label.into(),
                    session: nonce(),
                    nonce: nonce(),
                    generation: 1,
                    sequence: 0,
                };
                evidence.lock().expect("Evidence lock").sessions.insert(label.into(), session.clone());
                let navigation = evidence.clone();
                let popup = evidence.clone();
                let download = evidence.clone();
                let lifecycle = evidence.clone();
                let started = AtomicBool::new(false);
                let window = WebviewWindowBuilder::new(app, label, WebviewUrl::External("about:blank".parse()?))
                    .title(format!("{TITLE} — {label}"))
                    .visible(false)
                    .inner_size(600.0, 320.0)
                    .data_directory(root.join(directory))
                    .use_https_scheme(true)
                    .additional_browser_args(&format!("--remote-debugging-address=127.0.0.1 --remote-debugging-port={}", ports[index]))
                    .initialization_script(format!("Object.defineProperty(window,'__G5_SESSION__',{{value:Object.freeze({}),writable:false,configurable:false}});", serde_json::to_string(&session)?))
                    .on_navigation(move |url| {
                        let allowed = url.as_str() == "about:blank" || expected(label).is_some_and(|allowed| url.as_str() == allowed
                            || url.as_str() == allowed.replace("/index.html", "/download"));
                        if !allowed { navigation.lock().expect("Evidence lock").navigation_denied += 1; }
                        allowed
                    })
                    .on_new_window(move |_, _| {
                        popup.lock().expect("Evidence lock").popup_denied += 1;
                        NewWindowResponse::Deny
                    })
                    .on_download(move |_, _| {
                        download.lock().expect("Evidence lock").download_denied += 1;
                        false
                    })
                    .on_page_load(move |_, payload| {
                        if payload.event() == tauri::webview::PageLoadEvent::Started
                            && expected(label).is_some_and(|url| payload.url().as_str() == url)
                            && started.swap(true, Ordering::SeqCst)
                        {
                            let mut state = lifecycle.lock().expect("Evidence lock");
                            let session = state.sessions.get_mut(label).expect("Bound session");
                            session.generation = session.generation.checked_add(1).expect("Generation budget");
                            session.nonce = nonce();
                            session.sequence = 0;
                            state.sessions_revoked += 1;
                        }
                    })
                    .build()?;
                #[cfg(windows)]
                configure_native_policy(&window, root.join(format!("download-{directory}")), expected(label).expect("Fixed destination"), evidence.clone())?;
            }
            Ok(())
        })
        .run(context)
        .expect("Dedicated validation app");
}
