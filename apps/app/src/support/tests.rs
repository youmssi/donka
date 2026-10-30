use super::*;
use std::io::Read as _;

fn secrets(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

#[test]
fn secrets_tokens_and_people_are_removed() {
    let text = "\
DATABASE_URL_FILE=/run/secrets/database_url
DONKA_EXPLAIN_API_KEY=sk-live-0123456789
DONKA_REGISTRY_TOKEN: ${DONKA_REGISTRY_TOKEN:-}
smtp_url=smtp://mailer:Pa55word!@mail.bank.example:587?tls=required
2026-10-30T08:00:00Z INFO issued dnk_log_Abc123xyz789QWERTY for ada@bank.example
GET /en/setup-password?token=s3cr3t-link-token&x=1 from 41.202.219.70 via 172.18.0.5
Authorization: Bearer eyJhbGciOi.payload.sig
the decision log key is 5Gx4mUl0Q2Y4UmG2YxX3cHlWb0ZQ1N3l5Gx4mUl0Q2Y=
";
    let clean = redact(
        text,
        &secrets(&["5Gx4mUl0Q2Y4UmG2YxX3cHlWb0ZQ1N3l5Gx4mUl0Q2Y="]),
    );
    for leak in [
        "sk-live",
        "Pa55word",
        "dnk_log_",
        "ada@bank.example",
        "s3cr3t-link-token",
        "41.202.219.70",
        "eyJhbGciOi",
        "5Gx4mUl0Q2Y4",
    ] {
        assert!(!clean.contains(leak), "{leak} left in:\n{clean}");
    }
    // What support needs stays: file paths, interpolations, private addresses, hosts.
    for kept in [
        "DATABASE_URL_FILE=/run/secrets/database_url",
        "${DONKA_REGISTRY_TOKEN:-}",
        "smtp://[redacted]@mail.bank.example:587",
        "172.18.0.5",
        "token=[redacted]&x=1",
        "Bearer [redacted]",
    ] {
        assert!(clean.contains(kept), "{kept} missing from:\n{clean}");
    }
}

fn unpack(bytes: &[u8]) -> Vec<(String, String)> {
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(bytes));
    archive
        .entries()
        .unwrap()
        .map(|entry| {
            let mut entry = entry.unwrap();
            let name = entry.path().unwrap().to_string_lossy().into_owned();
            let mut text = String::new();
            entry.read_to_string(&mut text).unwrap();
            (name, text)
        })
        .collect()
}

/// A real installation's files, with secrets in an override and people in an upgrade report:
/// nothing secret, no token and no email address reaches the bundle.
#[test]
fn a_bundle_of_an_installation_holds_no_secret_token_or_person() {
    let dir = std::env::temp_dir().join(format!("donka-support-{}", uuid::Uuid::new_v4()));
    let mut installation = install::Installation::for_version("1.4.0");
    installation.administrator = "ada@bank.example".to_owned();
    installation.demo = true;
    install::init(
        &dir,
        &installation,
        &install::Provided::default(),
        |buf: &mut [u8]| getrandom::fill(buf).map_err(|err| err.to_string()),
    )
    .unwrap();
    std::fs::write(
        dir.join("docker-compose.override.yml"),
        "services:\n  app:\n    environment:\n      DONKA_STORAGE_SECRET_ACCESS_KEY: wJalrXUtnFEMI-K7MDENG\n",
    )
    .unwrap();
    std::fs::create_dir_all(dir.join("upgrades")).unwrap();
    std::fs::write(
        dir.join("upgrades/20261030-0800-1.4.0-to-1.5.0.txt"),
        "upgraded by grace@bank.example with token dnk_ci_ZXhhbXBsZS10b2tlbi12YWx1ZQ\n",
    )
    .unwrap();

    let values = secret_values(&dir.join(SECRETS)).unwrap();
    assert!(values.len() >= 5, "{values:?}");
    let entries: Vec<Entry> = files(&dir)
        .unwrap()
        .into_iter()
        .map(|entry| Entry {
            text: redact(&entry.text, &values),
            name: entry.name,
        })
        .collect();
    let unpacked = unpack(&archive(&readme(), &entries).unwrap());
    let names: Vec<&str> = unpacked.iter().map(|(name, _)| name.as_str()).collect();
    for expected in [
        "donka-support/README.txt",
        "donka-support/config/donka.yaml",
        "donka-support/config/.env",
        "donka-support/config/docker-compose.override.yml",
        "donka-support/secrets.txt",
        "donka-support/upgrades/20261030-0800-1.4.0-to-1.5.0.txt",
    ] {
        assert!(names.contains(&expected), "{expected} missing: {names:?}");
    }
    let everything: String = unpacked.iter().map(|(_, text)| text.as_str()).collect();
    for value in &values {
        assert!(
            !everything.contains(value.as_str()),
            "a secret value is in the bundle"
        );
    }
    for leak in [
        "wJalrXUtnFEMI",
        "dnk_ci_",
        "ada@bank.example",
        "grace@bank.example",
    ] {
        assert!(!everything.contains(leak), "{leak} is in the bundle");
    }
    // The secrets are named, so support knows which exist.
    assert!(everything.contains("decision_log_key\n"));
    std::fs::remove_dir_all(&dir).unwrap();
}
