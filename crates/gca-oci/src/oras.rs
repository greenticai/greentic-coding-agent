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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn maps_missing_binary_and_failed_command() {
        let mut missing = Command::new("definitely-not-oras-for-gca-tests");
        assert!(matches!(run_oras(&mut missing), Err(OrasError::Missing)));

        let mut failed = Command::new("false");
        assert!(
            matches!(run_oras(&mut failed), Err(OrasError::Failed(message)) if message.is_empty())
        );
    }

    #[test]
    fn redacts_login_token_from_stderr() {
        assert_eq!(
            redacted_output(b"token abc123 appeared twice: abc123", "abc123"),
            "token [redacted] appeared twice: [redacted]"
        );
        assert_eq!(redacted_output(b"plain failure", ""), "plain failure");
    }

    #[test]
    fn pull_push_and_login_invoke_oras_with_auth() {
        let _guard = ENV_LOCK.lock().expect("environment lock poisoned");
        let temp = tempfile::tempdir().expect("tempdir");
        let bin_dir = temp.path().join("bin");
        fs::create_dir(&bin_dir).expect("bin dir");
        let log_path = temp.path().join("oras.log");
        let stdin_path = temp.path().join("oras.stdin");
        let script_path = bin_dir.join("oras");
        fs::write(
            &script_path,
            format!(
                "#!/bin/sh\nprintf '%s\\n' \"$*\" >> '{}'\nif [ \"$1\" = login ]; then cat > '{}'; fi\nif [ \"${{ORAS_FAIL:-}}\" = 1 ]; then printf 'registry rejected secret-token\\n' >&2; exit 7; fi\nexit 0\n",
                log_path.display(),
                stdin_path.display()
            ),
        )
        .expect("write fake oras");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut permissions = fs::metadata(&script_path).expect("metadata").permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script_path, permissions).expect("chmod fake oras");
        }

        let original_path = std::env::var_os("PATH").unwrap_or_default();
        let original_fail = std::env::var_os("ORAS_FAIL");
        set_env("PATH", path_with_fake_oras(&bin_dir, &original_path));
        remove_env("ORAS_FAIL");

        let auth = RegistryAuth {
            registry: "ghcr.io".to_string(),
            username: None,
            token: "secret-token".to_string(),
        };
        let out_dir = temp.path().join("pulled");
        oras_pull("ghcr.io/acme/demo:latest", &out_dir, Some(&auth)).expect("pull succeeds");
        assert!(out_dir.exists());
        assert_eq!(
            fs::read_to_string(&stdin_path).expect("stdin"),
            "secret-token"
        );

        let push_dir = temp.path().join("push-source");
        fs::create_dir(&push_dir).expect("push source");
        oras_push("ghcr.io/acme/demo:latest", &push_dir, None).expect("push succeeds");

        let log = fs::read_to_string(&log_path).expect("log");
        assert!(log.contains("login ghcr.io -u greentic-agent --password-stdin"));
        assert!(log.contains(&format!(
            "pull ghcr.io/acme/demo:latest -o {}",
            out_dir.display()
        )));
        assert!(log.contains(&format!(
            "push ghcr.io/acme/demo:latest {}",
            push_dir.display()
        )));

        set_env("ORAS_FAIL", OsString::from("1"));
        let error = oras_login("ghcr.io", "agent", "secret-token").expect_err("login fails");
        assert!(
            matches!(error, OrasError::Failed(message) if message.contains("[redacted]") && !message.contains("secret-token"))
        );

        restore_env("ORAS_FAIL", original_fail);
        set_env("PATH", original_path);
    }

    fn path_with_fake_oras(bin_dir: &Path, original_path: &OsString) -> OsString {
        let mut paths = vec![bin_dir.to_path_buf()];
        paths.extend(std::env::split_paths(original_path));
        std::env::join_paths(paths).expect("join PATH")
    }

    fn set_env(key: &str, value: OsString) {
        // SAFETY: tests that mutate process environment hold ENV_LOCK for the full mutation window.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: tests that mutate process environment hold ENV_LOCK for the full mutation window.
        unsafe { std::env::remove_var(key) };
    }

    fn restore_env(key: &str, value: Option<OsString>) {
        if let Some(value) = value {
            set_env(key, value);
        } else {
            remove_env(key);
        }
    }
}
