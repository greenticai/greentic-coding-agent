use crate::RegistryAuth;
use std::path::Path;
use std::process::{Command, Stdio};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OrasError {
    #[error("oras is required for GHCR sync. Install it with: brew install oras")]
    Missing,
    #[error("oras command failed: {0}")]
    Failed(String),
    #[error("failed to run oras: {0}")]
    Io(#[from] std::io::Error),
}

pub fn oras_pull(
    reference: &str,
    out_dir: &Path,
    auth: Option<&RegistryAuth>,
) -> Result<(), OrasError> {
    if let Some(auth) = auth {
        oras_login(
            &auth.registry,
            auth.username.as_deref().unwrap_or("greentic-agent"),
            &auth.token,
        )?;
    }
    std::fs::create_dir_all(out_dir)?;
    run_oras(
        Command::new("oras")
            .arg("pull")
            .arg(reference)
            .arg("-o")
            .arg(out_dir),
    )
}

pub fn oras_push(
    reference: &str,
    dir: &Path,
    auth: Option<&RegistryAuth>,
) -> Result<(), OrasError> {
    if let Some(auth) = auth {
        oras_login(
            &auth.registry,
            auth.username.as_deref().unwrap_or("greentic-agent"),
            &auth.token,
        )?;
    }
    run_oras(Command::new("oras").arg("push").arg(reference).arg(dir))
}

pub fn oras_login(registry: &str, username: &str, token: &str) -> Result<(), OrasError> {
    let mut child = Command::new("oras")
        .arg("login")
        .arg(registry)
        .arg("-u")
        .arg(username)
        .arg("--password-stdin")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(map_missing)?;

    if let Some(mut stdin) = child.stdin.take() {
        use std::io::Write;
        stdin.write_all(token.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(OrasError::Failed(redacted_output(&output.stderr, token)))
}

fn run_oras(command: &mut Command) -> Result<(), OrasError> {
    let output = command.output().map_err(map_missing)?;
    if output.status.success() {
        return Ok(());
    }
    Err(OrasError::Failed(redacted_output(&output.stderr, "")))
}

fn map_missing(error: std::io::Error) -> OrasError {
    if error.kind() == std::io::ErrorKind::NotFound {
        OrasError::Missing
    } else {
        OrasError::Io(error)
    }
}

fn redacted_output(raw: &[u8], token: &str) -> String {
    let text = String::from_utf8_lossy(raw).to_string();
    if token.is_empty() {
        text
    } else {
        text.replace(token, "[redacted]")
    }
}
