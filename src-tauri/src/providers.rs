//! Authentication only: no model session, prompt, tool, or network summary is sent.
use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc, Mutex,
    },
    time::{Duration, Instant},
};

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatus {
    pub busy: bool,
    pub signed_in: bool,
    pub message: String,
    pub login_url: Option<String>,
    pub account: Option<String>,
    pub plan: Option<String>,
    pub client_version: Option<String>,
}
struct Slot {
    status: Arc<Mutex<AuthStatus>>,
    cancel: Arc<AtomicBool>,
}
pub struct Providers {
    root: PathBuf,
    slots: [Slot; 2],
}
fn index(provider: &str) -> Result<usize> {
    match provider {
        "chatgpt" => Ok(0),
        "grok" => Ok(1),
        _ => bail!("Unknown provider"),
    }
}
fn private_dir(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

// Resolve npm's native Codex payload directly so cancellation also stops its login server.
fn codex_payload(path: &Path) -> Option<PathBuf> {
    let real = path.canonicalize().ok()?;
    if real.extension().is_none_or(|e| e != "js") {
        return Some(real);
    }
    let package = real.parent()?.parent()?;
    let (platform, triple) = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", "aarch64") => ("darwin-arm64", "aarch64-apple-darwin"),
        ("macos", "x86_64") => ("darwin-x64", "x86_64-apple-darwin"),
        ("windows", "x86_64") => ("win32-x64", "x86_64-pc-windows-msvc"),
        ("windows", "aarch64") => ("win32-arm64", "aarch64-pc-windows-msvc"),
        ("linux", "x86_64") => ("linux-x64", "x86_64-unknown-linux-musl"),
        ("linux", "aarch64") => ("linux-arm64", "aarch64-unknown-linux-musl"),
        _ => return None,
    };
    let executable = if cfg!(windows) { "codex.exe" } else { "codex" };
    for root in [
        package.join("node_modules/@openai"),
        package.parent()?.to_path_buf(),
    ] {
        for sub in ["bin", "codex"] {
            let path = root.join(format!(
                "codex-{platform}/vendor/{triple}/{sub}/{executable}"
            ));
            if path.is_file() {
                return Some(path);
            }
        }
    }
    None
}
fn executable(provider: &str) -> Result<PathBuf> {
    let name = if provider == "chatgpt" {
        "codex"
    } else {
        "grok"
    };
    let mut candidates = vec![hns_core::tool_path(name)];
    if let Some(home) = std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }) {
        let home = PathBuf::from(home);
        candidates.extend([
            home.join(format!(
                ".{name}/bin/{name}{}",
                if cfg!(windows) { ".exe" } else { "" }
            )),
            home.join(format!(".local/bin/{name}")),
        ]);
        if provider == "chatgpt" {
            // fnm and nvm are common on machines where GUI PATH has no Node location.
            for (root, suffix) in [
                (
                    home.join(".local/share/fnm/node-versions"),
                    "installation/bin/codex",
                ),
                (home.join(".nvm/versions/node"), "bin/codex"),
            ] {
                if let Ok(entries) = std::fs::read_dir(root) {
                    let mut paths: Vec<_> =
                        entries.flatten().map(|e| e.path().join(suffix)).collect();
                    paths.sort();
                    paths.reverse();
                    candidates.extend(paths);
                }
            }
        }
    }
    if provider == "chatgpt" {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            candidates
                .push(PathBuf::from(appdata).join("npm/node_modules/@openai/codex/bin/codex.js"));
        }
    }
    for path in candidates {
        if path.is_file() {
            if provider == "chatgpt" {
                if let Some(path) = codex_payload(&path) {
                    return Ok(path);
                }
            } else {
                return Ok(path.canonicalize()?);
            }
        }
    }
    bail!("Install the official {name} client and restart the desktop app. See the provider setup guide.")
}
fn provider_command(root: &Path, provider: &str) -> Result<Command> {
    index(provider)?;
    private_dir(root)?;
    let profile = root.join(provider);
    let workspace = profile.join("empty-workspace");
    private_dir(&profile)?;
    private_dir(&workspace)?;
    let config = if provider == "chatgpt" {
        "forced_login_method = \"chatgpt\"\ncli_auth_credentials_store = \"file\"\n[analytics]\nenabled = false\n"
    } else {
        "[grok_com_config]\ndisable_api_key_auth = true\npreferred_method = \"oidc\"\n"
    };
    std::fs::write(profile.join("config.toml"), config)?;
    let mut command = Command::new(executable(provider)?);
    isolated_environment(&mut command);
    command
        .env(
            if provider == "chatgpt" {
                "CODEX_HOME"
            } else {
                "GROK_HOME"
            },
            profile,
        )
        .env("NO_COLOR", "1")
        .env("TERM", "dumb")
        .current_dir(workspace)
        .stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    Ok(command)
}
fn isolated_environment(command: &mut Command) {
    command.env_clear();
    // Preserve OS operation only. No ambient API keys, provider overrides, or agent context.
    for key in [
        "HOME",
        "USER",
        "LOGNAME",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
        "SystemRoot",
        "WINDIR",
        "TEMP",
        "TMP",
        "TMPDIR",
        "PATH",
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_RUNTIME_DIR",
        "DBUS_SESSION_BUS_ADDRESS",
    ] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Rpc {
    child: Process,
    rx: mpsc::Receiver<String>,
    next_id: u32,
}
impl Rpc {
    fn start(mut command: Command) -> Result<Self> {
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;
        let out = child.stdout.take().context("Missing client stdout")?;
        let (tx, rx) = mpsc::sync_channel(64);
        std::thread::spawn(move || {
            // Authentication exchanges have no reason to produce unbounded output.
            for line in BufReader::new(out.take(2 * 1024 * 1024)).lines() {
                let Ok(line) = line else {
                    break;
                };
                if line.len() > 256 * 1024 || tx.send(line).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child: Process(child),
            rx,
            next_id: 0,
        })
    }
    fn call(&mut self, method: &str, params: Value, cancel: &AtomicBool) -> Result<Value> {
        self.next_id += 1;
        let id = self.next_id;
        writeln!(
            self.child.0.stdin.as_mut().context("Client closed stdin")?,
            "{}",
            json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
        )?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            if cancel.load(Ordering::Relaxed) {
                bail!("Cancelled");
            }
            if Instant::now() >= deadline {
                bail!("Provider authentication check timed out");
            }
            match self.rx.recv_timeout(Duration::from_millis(100)) {
                Ok(line) => {
                    let value: Value =
                        serde_json::from_str(&line).context("Unsupported provider protocol")?;
                    if value["id"] == id && value.get("method").is_none() {
                        if value.get("error").is_some() {
                            bail!("Provider rejected authentication. Sign in again.");
                        }
                        return Ok(value["result"].clone());
                    }
                    if value.get("method").is_some() && value.get("id").is_some() {
                        // Authentication never grants file, terminal, tool, or permission requests.
                        writeln!(
                            self.child.0.stdin.as_mut().context("Client closed stdin")?,
                            "{}",
                            json!({"jsonrpc":"2.0","id":value["id"],"error":{"code":-32601,"message":"Authentication-only client"}})
                        )?;
                    }
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(_) => bail!("Provider client exited during authentication check"),
            }
        }
    }
}
fn check(root: &Path, provider: &str, cancel: &AtomicBool) -> Result<AuthStatus> {
    let mut command = provider_command(root, provider)?;
    command.args(if provider == "chatgpt" {
        vec!["app-server", "--listen", "stdio://"]
    } else {
        vec!["agent", "stdio"]
    });
    let mut rpc = Rpc::start(command)?;
    if provider == "chatgpt" {
        let init = rpc.call("initialize", json!({"clientInfo":{"name":"home_network_security","version":"0.1.0"},"capabilities":{}}), cancel)?;
        writeln!(
            rpc.child.0.stdin.as_mut().context("Client closed stdin")?,
            "{}",
            json!({"method":"initialized"})
        )?;
        let result = rpc.call("account/read", json!({"refreshToken":true}), cancel)?;
        let account = &result["account"];
        let signed_in = account["type"] == "chatgpt";
        Ok(AuthStatus {
            signed_in,
            message: if signed_in {
                "ChatGPT session available. No model request sent."
            } else {
                "Sign in with your ChatGPT account."
            }
            .into(),
            account: account["email"].as_str().map(str::to_string),
            plan: account["planType"].as_str().map(str::to_string),
            client_version: init["userAgent"].as_str().map(str::to_string),
            ..AuthStatus::default()
        })
    } else {
        let init = rpc.call("initialize", json!({"protocolVersion":1,"clientInfo":{"name":"home-network-security","version":"0.1.0"},"clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false}}), cancel)?;
        let cached = init["authMethods"]
            .as_array()
            .is_some_and(|methods| methods.iter().any(|m| m["id"] == "cached_token"));
        let mut status = AuthStatus {
            client_version: init["agentInfo"]["version"].as_str().map(str::to_string),
            message: "Sign in with your Grok account.".into(),
            ..AuthStatus::default()
        };
        if cached {
            let auth = rpc.call(
                "authenticate",
                json!({"methodId":"cached_token","_meta":{"headless":true}}),
                cancel,
            )?;
            status.signed_in = true;
            status.message = "Grok session available. No model request sent.".into();
            status.account = auth["_meta"]["email"].as_str().map(str::to_string);
            status.plan = auth["_meta"]["subscriptionTier"]
                .as_str()
                .map(str::to_string);
        }
        Ok(status)
    }
}
fn plain(text: &str) -> String {
    let mut result = String::new();
    let mut ansi = false;
    for c in text.chars() {
        if c == '\u{1b}' {
            ansi = true;
            continue;
        }
        if ansi {
            if c.is_ascii_alphabetic() {
                ansi = false;
            }
            continue;
        }
        if !c.is_control() || c == '\n' || c == '\t' {
            result.push(c);
        }
    }
    result
}
pub fn allowed_login_url(provider: &str, value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none()
        && match provider {
            "chatgpt" => matches!(url.host_str(), Some("auth.openai.com" | "chatgpt.com")),
            "grok" => matches!(
                url.host_str(),
                Some("auth.x.ai" | "accounts.x.ai" | "grok.com")
            ),
            _ => false,
        }
}
fn login(
    root: &Path,
    provider: &str,
    status: &Mutex<AuthStatus>,
    cancel: &AtomicBool,
) -> Result<()> {
    let mut command = provider_command(root, provider)?;
    command.args(["login", "--device-auth"]);
    let mut child = Process(
        command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?,
    );
    let (tx, rx) = mpsc::sync_channel(64);
    for stream in [
        Box::new(child.0.stdout.take().context("No stdout")?) as Box<dyn Read + Send>,
        Box::new(child.0.stderr.take().context("No stderr")?) as Box<dyn Read + Send>,
    ] {
        let tx = tx.clone();
        std::thread::spawn(move || {
            for line in BufReader::new(stream.take(65536)).lines() {
                if let Ok(line) = line {
                    if tx.send(plain(&line)).is_err() {
                        break;
                    }
                } else {
                    break;
                }
            }
        });
    }
    drop(tx);
    let deadline = Instant::now() + Duration::from_secs(900);
    loop {
        if cancel.load(Ordering::Relaxed) {
            bail!("Sign-in cancelled. The code may remain valid until the provider expires it.");
        }
        if Instant::now() >= deadline {
            bail!("Sign-in timed out. Request a new code.");
        }
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(line) => {
                let mut status = status.lock().map_err(|e| anyhow::anyhow!(e.to_string()))?;
                if status.message.len() + line.len() < 16384 {
                    status.message.push_str(&line);
                    status.message.push('\n');
                }
                for word in line.split_whitespace() {
                    if allowed_login_url(provider, word) {
                        status.login_url = Some(word.into());
                    }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => {
                if let Some(exit) = child.0.try_wait()? {
                    if !exit.success() {
                        bail!("Sign-in did not complete. See the provider message above.");
                    }
                    return Ok(());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}
impl Providers {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            slots: std::array::from_fn(|_| Slot {
                status: Arc::new(Mutex::new(AuthStatus::default())),
                cancel: Arc::new(AtomicBool::new(false)),
            }),
        }
    }
    pub fn status(&self, provider: &str) -> Result<AuthStatus> {
        Ok(self.slots[index(provider)?]
            .status
            .lock()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            .clone())
    }
    pub fn shutdown(&self) {
        self.cancel_all();
        let deadline = Instant::now() + Duration::from_secs(5);
        while self
            .slots
            .iter()
            .any(|slot| slot.status.lock().is_ok_and(|s| s.busy))
            && Instant::now() < deadline
        {
            std::thread::sleep(Duration::from_millis(50));
        }
    }
    pub fn cancel_all(&self) {
        for slot in &self.slots {
            slot.cancel.store(true, Ordering::Relaxed);
        }
    }
    pub fn action(&self, provider: String, action: String) -> Result<()> {
        let slot = &self.slots[index(&provider)?];
        if action == "cancel" {
            slot.cancel.store(true, Ordering::Relaxed);
            return Ok(());
        }
        if !matches!(action.as_str(), "login" | "check" | "logout") {
            bail!("Unknown authentication action");
        }
        let mut status = slot
            .status
            .lock()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        if status.busy {
            bail!("An authentication operation is already running");
        }
        *status = AuthStatus {
            busy: true,
            message: "Contacting the official provider client…\n".into(),
            ..AuthStatus::default()
        };
        slot.cancel.store(false, Ordering::Relaxed);
        let (root, shared, cancel) = (self.root.clone(), slot.status.clone(), slot.cancel.clone());
        std::thread::spawn(move || {
            let result = (|| -> Result<AuthStatus> {
                if action == "login" {
                    login(&root, &provider, &shared, &cancel)?;
                }
                if action == "logout" {
                    let mut command = provider_command(&root, &provider)?;
                    command.arg("logout");
                    hns_core::bounded_output_cancellable(
                        &mut command,
                        65536,
                        Duration::from_secs(15),
                        &cancel,
                    )?;
                }
                check(&root, &provider, &cancel)
            })();
            if let Ok(mut status) = shared.lock() {
                match result {
                    Ok(value) => *status = value,
                    Err(e) => {
                        status.busy = false;
                        status.signed_in = false;
                        status.login_url = None;
                        if cancel.load(Ordering::Relaxed) {
                            status.message = e.to_string();
                        } else {
                            status.message.push_str(&format!("\n{e}"));
                        }
                    }
                }
            }
        });
        Ok(())
    }
}
impl Drop for Providers {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn login_links_reject_unrelated_or_credential_urls() {
        assert!(allowed_login_url(
            "chatgpt",
            "https://auth.openai.com/codex/device"
        ));
        assert!(allowed_login_url(
            "grok",
            "https://auth.x.ai/device?user_code=TEST"
        ));
        for url in [
            "https://auth.x.ai.evil.test/",
            "https://user@auth.x.ai/",
            "file:///etc/passwd",
            "http://auth.x.ai/",
            "https://auth.x.ai:8080/",
        ] {
            assert!(!allowed_login_url("grok", url));
        }
        assert!(!allowed_login_url("chatgpt", "https://auth.x.ai/"));
    }
    #[test]
    fn terminal_output_is_plain_text() {
        assert_eq!(plain("\x1b[31mA test code\x1b[0m\r"), "A test code");
    }
    #[test]
    fn providers_are_explicit() {
        assert!(index("api").is_err());
        assert!(index("grok").is_ok());
    }
    #[test]
    #[ignore = "Requires installed official clients; does not sign in or send a prompt"]
    fn installed_client_protocols() {
        let root = tempfile::tempdir().unwrap();
        for provider in ["chatgpt", "grok"] {
            let result = check(root.path(), provider, &AtomicBool::new(false)).unwrap();
            assert!(
                !result.signed_in,
                "A fresh profile must not inherit a session"
            );
            assert!(result.account.is_none());
            println!("{provider}: isolated signed-out protocol verified");
        }
    }
    #[test]
    fn subprocess_environment_uses_an_allowlist() {
        let mut command = Command::new("synthetic-client");
        isolated_environment(&mut command);
        let keys: Vec<_> = command
            .get_envs()
            .map(|(k, _)| k.to_string_lossy().to_string())
            .collect();
        for forbidden in [
            "OPENAI_API_KEY",
            "XAI_API_KEY",
            "CODEX_ACCESS_TOKEN",
            "GROK_CONFIG",
            "GROK_CODE_XAI_API_KEY",
        ] {
            assert!(!keys.iter().any(|k| k == forbidden));
        }
    }
    #[test]
    #[cfg(unix)]
    fn rpc_denies_unsolicited_permissions_before_returning_result() {
        let mut fake = Command::new("python3");
        fake.args(["-c", r#"import json,sys
request=json.loads(input())
print(json.dumps({'jsonrpc':'2.0','id':99,'method':'session/request_permission','params':{}}),flush=True)
reply=json.loads(input())
assert reply['id']==99 and reply['error']['code']==-32601
print(json.dumps({'jsonrpc':'2.0','id':request['id'],'result':{'denied':True}}),flush=True)
"#]);
        let mut rpc = Rpc::start(fake).unwrap();
        let result = rpc
            .call("initialize", json!({}), &AtomicBool::new(false))
            .unwrap();
        assert_eq!(result["denied"], true);
    }
    #[test]
    #[cfg(unix)]
    fn rpc_cancellation_does_not_wait_for_provider_output() {
        let mut fake = Command::new("python3");
        fake.args(["-c", "import time; time.sleep(30)"]);
        let mut rpc = Rpc::start(fake).unwrap();
        let start = Instant::now();
        assert!(rpc
            .call("initialize", json!({}), &AtomicBool::new(true))
            .is_err());
        drop(rpc);
        assert!(start.elapsed() < Duration::from_secs(2));
    }
}
