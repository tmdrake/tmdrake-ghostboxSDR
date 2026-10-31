//! Windows helper: locate and run the WinUSB package script.
//! No kernel driver is compiled — the script binds inbox WinUSB via Zadig.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn find_script() -> Option<PathBuf> {
    let mut dirs = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            dirs.push(parent.to_path_buf());
            let mut cur = parent.to_path_buf();
            for _ in 0..6 {
                match cur.parent() {
                    Some(p) => {
                        cur = p.to_path_buf();
                        dirs.push(cur.clone());
                    }
                    None => break,
                }
            }
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        dirs.push(cwd);
    }
    for dir in dirs {
        for rel in ["packaging/windows/install-winusb.ps1", "install-winusb.ps1"] {
            let candidate = dir.join(rel);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn powershell() -> Command {
    let mut cmd = Command::new("powershell");
    cmd.arg("-NoProfile").arg("-ExecutionPolicy").arg("Bypass");
    cmd
}

fn run_script(script: &Path, extra: &[&str]) -> anyhow::Result<()> {
    let mut cmd = powershell();
    cmd.arg("-File").arg(script);
    for a in extra {
        cmd.arg(a);
    }
    let status = cmd.status()?;
    if !status.success() {
        anyhow::bail!("WinUSB package exited with {status}");
    }
    Ok(())
}

pub fn print_status() -> anyhow::Result<()> {
    match find_script() {
        Some(script) => run_script(&script, &["-Status"]),
        None => {
            println!("WinUSB package script not found (packaging/windows/install-winusb.ps1).");
            println!("Plug the dongle in and install WinUSB on Bulk-In Interface 0 with Zadig.");
            Ok(())
        }
    }
}

pub fn setup() -> anyhow::Result<()> {
    let script = find_script().ok_or_else(|| {
        anyhow::anyhow!(
            "WinUSB package script not found. From the repo: powershell -ExecutionPolicy Bypass -File packaging/windows/install-winusb.ps1"
        )
    })?;
    println!("Starting WinUSB package: {}", script.display());
    run_script(&script, &["-Install"])
}

/// Launch the installer without waiting (for the GUI button).
pub fn start_installer() -> anyhow::Result<PathBuf> {
    let script = find_script().ok_or_else(|| {
        anyhow::anyhow!("WinUSB package script not found next to the app or in the repo")
    })?;
    let mut cmd = powershell();
    cmd.arg("-File").arg(&script).arg("-Install");
    cmd.spawn()?;
    Ok(script)
}
