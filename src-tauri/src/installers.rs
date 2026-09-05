//! Operator-visible installation, with fixed package IDs and commands only.
use anyhow::{bail, Result};
use std::{process::Command, time::Duration};
fn install_script(platform: &str, tool: &str) -> Result<String> {
    let package = match tool {
        "nmap" => "nmap",
        "tshark" => "wireshark",
        "capture-permission" => "wireshark-chmodbpf",
        _ => bail!("Unknown collection tool"),
    };
    match platform {
        "macos" => {
            let brew = if cfg!(target_arch = "aarch64") {
                "/opt/homebrew/bin/brew"
            } else {
                "/usr/local/bin/brew"
            };
            let install = if tool == "capture-permission" {
                format!("{brew} install --cask {package}")
            } else {
                format!("{brew} install {package}")
            };
            Ok(format!("if [ ! -x {brew} ]; then /bin/bash -c \"$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)\"; fi; {install}"))
        }
        "windows" => {
            if tool == "nmap" {
                // The winget Insecure.Nmap entry still ships 7.80. Pin the current official installer.
                Ok("$ErrorActionPreference='Stop'; $hnsInstaller=Join-Path $env:TEMP ('hns-nmap-'+[guid]::NewGuid()+'.exe'); try { Invoke-WebRequest -UseBasicParsing -Uri 'https://nmap.org/dist/nmap-7.991-setup.exe' -OutFile $hnsInstaller; if ((Get-FileHash $hnsInstaller -Algorithm SHA256).Hash -ne '93BFD37BDB31A7ADFD932BEB5DBCE06025DA691D01A0939E806EA704F7367657') { throw 'Nmap installer checksum mismatch' }; Start-Process -FilePath $hnsInstaller -Wait } finally { Remove-Item -LiteralPath $hnsInstaller -ErrorAction SilentlyContinue }".into())
            } else {
                Ok("winget install --exact --id WiresharkFoundation.Wireshark --source winget --interactive".into())
            }
        }
        _ => bail!("Use your system package manager to install Nmap and Wireshark/TShark"),
    }
}
pub fn install(tool: &str) -> Result<bool> {
    let platform = std::env::consts::OS;
    let script = install_script(platform, tool)?;
    let description = format!("Open a terminal to install {}?\n\n{}\n\nComplete the package manager and administrator prompts. On Windows, select TShark and Npcap in the Wireshark installer. When finished, return here and click Continue. macOS may first install Homebrew if it is missing.", if tool == "capture-permission" { "Wireshark capture permissions" } else { tool }, script);
    let confirmed = rfd::MessageDialog::new()
        .set_title("Install collection tools")
        .set_description(description)
        .set_buttons(rfd::MessageButtons::OkCancel)
        .show();
    if confirmed != rfd::MessageDialogResult::Ok {
        return Ok(false);
    }
    if platform == "macos" {
        let escaped = script.replace('\\', "\\\\").replace('"', "\\\"");
        let mut command = Command::new("/usr/bin/osascript");
        command.args([
            "-e",
            &format!("tell application \"Terminal\" to do script \"{escaped}\""),
            "-e",
            "tell application \"Terminal\" to activate",
        ]);
        hns_core::bounded_output(&mut command, 65536, Duration::from_secs(15))?;
    } else {
        let mut command = Command::new("powershell.exe");
        command.args(["-NoProfile", "-NoExit", "-Command", &script]);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x00000010); // Interactive installer console, separate from the app.
        }
        let mut child = command.spawn()?;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
    }
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn installers_only_accept_fixed_packages() {
        for platform in ["macos", "windows"] {
            assert!(install_script(platform, "nmap; whoami").is_err());
            assert!(install_script(platform, "tshark")
                .unwrap()
                .contains(if platform == "macos" {
                    "brew install wireshark"
                } else {
                    "--id WiresharkFoundation.Wireshark --source winget --interactive"
                }));
        }
        assert!(install_script("linux", "nmap").is_err());
    }
}
