//! Clipboard commands use stdin so repository contents never become shell code.
use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};

pub fn copy(text: &str) -> Result<()> {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(target_os = "windows") {
        &[(
            "powershell.exe",
            &[
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "[Console]::InputEncoding = [System.Text.UTF8Encoding]::new(); Set-Clipboard -Value ([Console]::In.ReadToEnd())",
            ],
        )]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    };
    let mut failures = Vec::new();
    for (program, args) in candidates {
        let mut child = match Command::new(program)
            .args(*args)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(child) => child,
            Err(err) => {
                failures.push(format!("{program}: {err}"));
                continue;
            }
        };
        // Closing stdin before waiting is required by clipboard helpers.
        let write = child
            .stdin
            .take()
            .expect("piped stdin")
            .write_all(text.as_bytes());
        let output = child
            .wait_with_output()
            .context("waiting for clipboard command")?;
        if output.status.success() && write.is_ok() {
            return Ok(());
        }
        failures.push(format!(
            "{program}: {}{}",
            String::from_utf8_lossy(&output.stderr).trim(),
            write.err().map(|e| format!(" ({e})")).unwrap_or_default()
        ));
    }
    bail!(
        "could not copy to clipboard; use -o packed.md instead. {}",
        failures.join("; ")
    )
}
