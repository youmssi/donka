//! The staging Runtime an installation starts with is wired at Studio's first start (DNK-64):
//! Studio issues its decision-log token and writes it to a file the Runtime reads
//! (`DECISION_LOG__TOKEN_FILE`), so nobody copies a token by hand.

use donka_decision_log::DecisionLog;
use donka_identity::Identity;
use donka_release::Environment;
use std::io::Write as _;
use std::path::Path;

/// The name the token is listed under, among the decision-log tokens administrators see.
pub const TOKEN_NAME: &str = "Staging Runtime (installed)";

#[derive(Debug, PartialEq, Eq)]
pub enum Wired {
    /// The file already holds a token: nothing was issued.
    Already,
    /// A token was issued and written.
    Issued,
}

/// Issues the staging Runtime's decision-log token on behalf of the first administrator and
/// writes it to `file`, unless the file already holds one. The token is written readable by
/// Studio's user only; the Runtime image runs as the same user.
pub async fn runtime_log_token(
    identity: &Identity,
    decision_log: &DecisionLog,
    file: &Path,
    admin_email: Option<&str>,
) -> Result<Wired, String> {
    if std::fs::read_to_string(file).is_ok_and(|token| !token.trim().is_empty()) {
        return Ok(Wired::Already);
    }
    let email = admin_email
        .ok_or("DONKA_BOOTSTRAP_ADMIN_EMAIL is not set: no administrator to issue it as")?;
    let admin = identity
        .find_by_email(email)
        .await
        .map_err(|err| err.to_string())?
        .filter(|user| user.is_admin)
        .ok_or_else(|| format!("{email} is not an administrator"))?;
    let issued = decision_log
        .issue_token(&admin, Environment::Staging, TOKEN_NAME)
        .await
        .map_err(|err| err.to_string())?;
    write_private(file, &issued.token)
        .map_err(|err| format!("cannot write {}: {err}", file.display()))?;
    Ok(Wired::Issued)
}

/// Writes `content` to a temporary file readable by its owner only, then renames it, so the
/// Runtime never reads half a token.
fn write_private(file: &Path, content: &str) -> std::io::Result<()> {
    let dir = file.parent().unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(dir)?;
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();
    let temp = dir.join(format!(".{name}.tmp"));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        options.mode(0o600);
    }
    let mut out = options.open(&temp)?;
    out.write_all(content.as_bytes())?;
    out.sync_all()?;
    std::fs::rename(&temp, file)
}
