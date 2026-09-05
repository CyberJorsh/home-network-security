#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod collection;
mod explanations;
mod installers;
mod providers;
use hns_core::*;
use serde::{Deserialize, Serialize};
use std::{
    io::Read,
    sync::{Arc, Mutex},
};
use tauri::{Manager, State};

struct AppState {
    local: Mutex<Store>,
    demo: Mutex<Store>,
    remote: Mutex<Option<Remote>>,
    collection: collection::Collection,
    providers: providers::Providers,
    explanations: explanations::Explanations,
}
#[derive(Clone, Deserialize)]
struct Remote {
    port: u16,
    token: String,
}
type CmdResult<T> = Result<T, String>;
fn error(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn remote_request(
    remote: &Remote,
    path: &str,
    body: Option<serde_json::Value>,
) -> CmdResult<serde_json::Value> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .redirect(reqwest::redirect::Policy::none())
        .no_proxy()
        .build()
        .map_err(error)?;
    let url = format!("http://127.0.0.1:{}{path}", remote.port);
    let request = if let Some(body) = body {
        client.post(url).json(&body)
    } else {
        client.get(url)
    };
    let response = request.bearer_auth(&remote.token).send().map_err(error)?;
    let status = response.status();
    let mut bytes = Vec::new();
    std::io::Read::take(response, 32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(error)?;
    if bytes.len() > 32 * 1024 * 1024 {
        return Err("Collector response exceeds 32 MiB".into());
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(error)?;
    if !status.is_success() {
        return Err(value["error"]
            .as_str()
            .unwrap_or("Collector request failed")
            .into());
    }
    Ok(value)
}

fn snapshot_impl(state: &AppState, mode: String, sensor: Option<String>) -> CmdResult<Snapshot> {
    if mode == "demo" {
        return state
            .demo
            .lock()
            .map_err(error)?
            .snapshot(sensor.as_deref(), "demo")
            .map_err(error);
    }
    if let Some(remote) = state.remote.lock().map_err(error)?.clone() {
        let path = if let Some(sensor) = sensor {
            identifier(&sensor).map_err(error)?;
            format!("/v1/snapshot?sensor={sensor}")
        } else {
            "/v1/snapshot".into()
        };
        return serde_json::from_value(remote_request(&remote, &path, None)?).map_err(error);
    }
    state
        .local
        .lock()
        .map_err(error)?
        .snapshot(sensor.as_deref(), "local")
        .map_err(error)
}
fn rename_device_impl(state: &AppState, mode: String, id: String, name: String) -> CmdResult<()> {
    if mode == "demo" {
        return state
            .demo
            .lock()
            .map_err(error)?
            .rename(&id, &name)
            .map_err(error);
    }
    if let Some(remote) = state.remote.lock().map_err(error)?.clone() {
        remote_request(
            &remote,
            "/v1/rename",
            Some(serde_json::json!({"id":id,"name":name})),
        )?;
        return Ok(());
    }
    state
        .local
        .lock()
        .map_err(error)?
        .rename(&id, &name)
        .map_err(error)
}
fn acknowledge_alert_impl(state: &AppState, mode: String, id: String) -> CmdResult<()> {
    if mode == "demo" {
        return state
            .demo
            .lock()
            .map_err(error)?
            .acknowledge(&id)
            .map_err(error);
    }
    if let Some(remote) = state.remote.lock().map_err(error)?.clone() {
        remote_request(
            &remote,
            "/v1/acknowledge",
            Some(serde_json::json!({"id":id})),
        )?;
        return Ok(());
    }
    state
        .local
        .lock()
        .map_err(error)?
        .acknowledge(&id)
        .map_err(error)
}
fn connect_collector_impl(state: &AppState, port: u16, token: String) -> CmdResult<()> {
    if token.trim().len() < 32 {
        return Err("Enter the collector token (at least 32 characters)".into());
    }
    let remote = Remote {
        port,
        token: token.trim().into(),
    };
    let _: Snapshot =
        serde_json::from_value(remote_request(&remote, "/v1/snapshot", None)?).map_err(error)?;
    *state.remote.lock().map_err(error)? = Some(remote);
    Ok(())
}
fn disconnect_collector_impl(state: &AppState) -> CmdResult<()> {
    *state.remote.lock().map_err(error)? = None;
    Ok(())
}
fn configure_networks_impl(state: &AppState, cidrs: String) -> CmdResult<()> {
    state
        .local
        .lock()
        .map_err(error)?
        .set_networks(&cidrs)
        .map_err(error)
}
fn import_file_impl(state: &AppState) -> CmdResult<Option<ImportResult>> {
    let Some(file) = rfd::FileDialog::new()
        .add_filter("Network observations", &["ndjson", "pcap", "pcapng", "xml"])
        .pick_file()
    else {
        return Ok(None);
    };
    let sensor_id = if file.extension().is_some_and(|e| e == "xml") {
        "discovery"
    } else {
        "import"
    };
    let mut store = state.local.lock().map_err(error)?;
    if !store
        .sensors()
        .map_err(error)?
        .iter()
        .any(|s| s.id == sensor_id)
    {
        store
            .set_sensor(&Sensor::new(sensor_id, "file"))
            .map_err(error)?;
    }
    let n = match file.extension().and_then(|s| s.to_str()) {
        Some("pcap" | "pcapng") => import_pcap(&mut store, &file, sensor_id).map_err(error)?,
        Some("xml") => {
            if std::fs::metadata(&file).map_err(error)?.len() > 16 * 1024 * 1024 {
                return Err("XML import exceeds 16 MiB".into());
            }
            let found =
                parse_nmap(&std::fs::read_to_string(file).map_err(error)?).map_err(error)?;
            store.save_discovery(sensor_id, &found).map_err(error)?;
            found.len()
        }
        Some("ndjson") => {
            if std::fs::metadata(&file).map_err(error)?.len() > 32 * 1024 * 1024 {
                return Err("Import exceeds 32 MiB".into());
            }
            store
                .import_json(&std::fs::read_to_string(file).map_err(error)?, sensor_id)
                .map_err(error)?
        }
        _ => return Err("Unsupported file type".into()),
    };
    *state.remote.lock().map_err(error)? = None;
    Ok(Some(ImportResult {
        count: n,
        sensor_id: sensor_id.into(),
    }))
}
fn open_provider_impl(provider: String) -> CmdResult<()> {
    let url = match provider.as_str() {
        "chatgpt" => "https://chatgpt.com/",
        "grok" => "https://grok.com/",
        _ => return Err("Unknown provider".into()),
    };
    open_url(url)
}
fn open_url(url: &str) -> CmdResult<()> {
    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(url).status();
    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("rundll32")
        .args(["url.dll,FileProtocolHandler", url])
        .status();
    #[cfg(target_os = "linux")]
    let status = std::process::Command::new("xdg-open").arg(url).status();
    if !status.map_err(error)?.success() {
        return Err("Could not open the provider website".into());
    }
    Ok(())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResult {
    count: usize,
    sensor_id: String,
}

// Keep blocking SQLite, dialogs, subprocesses and HTTP off the webview and async workers.
#[tauri::command]
async fn snapshot(
    state: State<'_, Arc<AppState>>,
    mode: String,
    sensor: Option<String>,
) -> CmdResult<Snapshot> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || snapshot_impl(&state, mode, sensor))
        .await
        .map_err(error)?
}
#[tauri::command]
async fn rename_device(
    state: State<'_, Arc<AppState>>,
    mode: String,
    id: String,
    name: String,
) -> CmdResult<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || rename_device_impl(&state, mode, id, name))
        .await
        .map_err(error)?
}
#[tauri::command]
async fn acknowledge_alert(
    state: State<'_, Arc<AppState>>,
    mode: String,
    id: String,
) -> CmdResult<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || acknowledge_alert_impl(&state, mode, id))
        .await
        .map_err(error)?
}
#[tauri::command]
async fn connect_collector(
    state: State<'_, Arc<AppState>>,
    port: u16,
    token: String,
) -> CmdResult<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || connect_collector_impl(&state, port, token))
        .await
        .map_err(error)?
}
#[tauri::command]
async fn disconnect_collector(state: State<'_, Arc<AppState>>) -> CmdResult<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || disconnect_collector_impl(&state))
        .await
        .map_err(error)?
}
#[tauri::command]
async fn configure_networks(state: State<'_, Arc<AppState>>, cidrs: String) -> CmdResult<()> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || configure_networks_impl(&state, cidrs))
        .await
        .map_err(error)?
}
#[tauri::command]
async fn import_file(state: State<'_, Arc<AppState>>) -> CmdResult<Option<ImportResult>> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || import_file_impl(&state))
        .await
        .map_err(error)?
}
#[tauri::command]
async fn open_provider(provider: String) -> CmdResult<()> {
    tauri::async_runtime::spawn_blocking(move || open_provider_impl(provider))
        .await
        .map_err(error)?
}

#[tauri::command]
async fn inspect_host() -> CmdResult<collection::Host> {
    tauri::async_runtime::spawn_blocking(collection::inspect)
        .await
        .map_err(error)?
}
#[tauri::command]
fn collection_status(state: State<'_, Arc<AppState>>) -> CmdResult<collection::Job> {
    Ok(state.collection.job.lock().map_err(error)?.clone())
}
#[tauri::command]
fn start_collection(
    state: State<'_, Arc<AppState>>,
    kind: String,
    target: String,
    seconds: u64,
    services: Option<bool>,
) -> CmdResult<String> {
    let id = state
        .collection
        .start(&kind, target, seconds, services.unwrap_or(false))?;
    *state.remote.lock().map_err(error)? = None;
    Ok(id)
}
#[tauri::command]
fn stop_capture(state: State<'_, Arc<AppState>>) {
    state.collection.stop();
}
#[tauri::command]
fn auth_status(
    state: State<'_, Arc<AppState>>,
    provider: String,
) -> CmdResult<providers::AuthStatus> {
    state.providers.status(&provider).map_err(error)
}
#[tauri::command]
fn auth_action(state: State<'_, Arc<AppState>>, provider: String, action: String) -> CmdResult<()> {
    state.providers.action(provider, action).map_err(error)
}
#[tauri::command]
async fn open_login(state: State<'_, Arc<AppState>>, provider: String) -> CmdResult<()> {
    let url = state
        .providers
        .status(&provider)
        .map_err(error)?
        .login_url
        .ok_or("No active sign-in URL")?;
    if !providers::allowed_login_url(&provider, &url) {
        return Err("Unsupported sign-in URL".into());
    }
    tauri::async_runtime::spawn_blocking(move || open_url(&url))
        .await
        .map_err(error)?
}

#[tauri::command]
async fn install_collection_tool(tool: String) -> CmdResult<bool> {
    tauri::async_runtime::spawn_blocking(move || installers::install(&tool).map_err(error))
        .await
        .map_err(error)?
}
#[tauri::command]
async fn provider_models(
    state: State<'_, Arc<AppState>>,
    provider: String,
) -> CmdResult<explanations::Catalog> {
    let state = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        explanations::catalog(&state.providers, &provider).map_err(error)
    })
    .await
    .map_err(error)?
}
#[tauri::command]
fn send_explanation(
    state: State<'_, Arc<AppState>>,
    request: explanations::Request,
) -> CmdResult<()> {
    state
        .explanations
        .start(&state.providers, request)
        .map_err(error)
}
#[tauri::command]
fn explanation_status(state: State<'_, Arc<AppState>>) -> CmdResult<explanations::Output> {
    Ok(state.explanations.output.lock().map_err(error)?.clone())
}
#[tauri::command]
fn stop_explanation(state: State<'_, Arc<AppState>>) {
    state.explanations.stop();
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            let root = app.path().app_local_data_dir()?;
            let path = root.join("network.db");
            app.manage(Arc::new(AppState {
                local: Mutex::new(Store::open(&path)?),
                demo: Mutex::new(demo_store()?),
                remote: Mutex::new(None),
                collection: collection::Collection::new(path),
                providers: providers::Providers::new(root.join("providers")),
                explanations: explanations::Explanations::new(),
            }));
            Ok(())
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                let state = window.state::<Arc<AppState>>();
                state.collection.stop();
                state.providers.cancel_all();
                state.collection.shutdown();
                state.explanations.shutdown();
                state.providers.shutdown();
            }
        })
        .invoke_handler(tauri::generate_handler![
            install_collection_tool,
            provider_models,
            send_explanation,
            explanation_status,
            stop_explanation,
            snapshot,
            rename_device,
            acknowledge_alert,
            connect_collector,
            disconnect_collector,
            configure_networks,
            import_file,
            open_provider,
            inspect_host,
            collection_status,
            start_collection,
            stop_capture,
            auth_status,
            auth_action,
            open_login
        ])
        .run(tauri::generate_context!())
        .expect("Unable to start Home Network Security");
}
