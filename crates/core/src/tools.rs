use std::{path::PathBuf, process::Command};

/// GUI launches often omit package-manager locations from PATH.
pub fn tool_path(name: &str) -> PathBuf {
    let filename = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    };
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .filter(|p| p.is_absolute())
                .collect()
        })
        .unwrap_or_default();
    if cfg!(target_os = "macos") {
        dirs.extend(
            [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/Applications/Wireshark.app/Contents/MacOS",
            ]
            .map(PathBuf::from),
        );
    }
    if cfg!(windows) {
        for key in ["ProgramFiles", "ProgramFiles(x86)"] {
            if let Some(root) = std::env::var_os(key) {
                dirs.extend(["Wireshark", "Nmap"].map(|p| PathBuf::from(&root).join(p)));
            }
        }
    }
    dirs.into_iter()
        .map(|d| d.join(&filename))
        .find(|p| p.is_file())
        .unwrap_or_else(|| filename.into())
}

pub fn tool_command(name: &str) -> Command {
    let mut command = Command::new(tool_path(name));
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000); // No console window for background tools.
    }
    command.env("LC_ALL", "C");
    command
}
