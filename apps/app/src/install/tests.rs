use super::*;

fn dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("donka-init-{name}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn system_random(buf: &mut [u8]) -> Result<(), String> {
    getrandom::fill(buf).map_err(|err| err.to_string())
}

fn value(env: &str, key: &str) -> String {
    env.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")))
        .unwrap_or_else(|| panic!("{key} missing"))
        .to_owned()
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap()
}

fn mode(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::metadata(path).unwrap().permissions().mode() & 0o777
}

const GENERATED: [&str; 5] = [
    POSTGRES_PASSWORD,
    DATABASE_URL,
    DECISION_LOG_KEY,
    SECOND_STEP_KEY,
    PERSON_INDEX_KEY,
];

#[test]
fn a_new_installation_keeps_its_secrets_in_files_only() {
    let dir = dir("fresh");
    init(
        &dir,
        &Installation::for_version("1.4.0"),
        &Provided::default(),
        system_random,
    )
    .unwrap();
    assert_eq!(read(&dir, "docker-compose.yml"), COMPOSE);
    let env = read(&dir, ".env");
    assert_eq!(value(&env, "DONKA_VERSION"), "1.4.0");
    assert_eq!(value(&env, "DONKA_RUNTIME_VERSION"), "latest");
    assert_eq!(value(&env, "DONKA_INSTALLATION_ROLE"), "production");
    assert_eq!(
        value(&env, "DATABASE_URL_FILE"),
        "/run/secrets/database_url"
    );
    assert_eq!(value(&env, "DONKA_COOKIE_SECURE"), "false");
    let secrets = dir.join(SECRETS);
    let password = read_secret(&secrets.join(POSTGRES_PASSWORD)).unwrap();
    assert_eq!(password.len(), 48);
    assert_eq!(
        read_secret(&secrets.join(DATABASE_URL)).unwrap(),
        format!("postgres://donka:{password}@postgres:5432/donka")
    );
    for key in [DECISION_LOG_KEY, SECOND_STEP_KEY, PERSON_INDEX_KEY] {
        let key = read_secret(&secrets.join(key)).unwrap();
        assert_eq!(B64.decode(key).unwrap().len(), 32);
    }
    // No secret value appears in a file Compose or anyone reads.
    for name in [".env", "runtime.env", CONFIG] {
        let text = read(&dir, name);
        for secret in GENERATED {
            let secret = read_secret(&secrets.join(secret)).unwrap();
            assert!(!text.contains(&secret), "{name} holds a secret");
        }
    }
    // Only the owner enters the folder; containers read a secret by name, never list them.
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(mode(&secrets), 0o711);
    assert_eq!(mode(&secrets.join(POSTGRES_PASSWORD)), 0o644);
    assert_eq!(mode(&dir.join(CONFIG)), 0o600);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_installation_gets_its_own_secrets() {
    let (a, b) = (dir("a"), dir("b"));
    let installation = Installation::for_version("1.4.0");
    init(&a, &installation, &Provided::default(), system_random).unwrap();
    init(&b, &installation, &Provided::default(), system_random).unwrap();
    let secret = |d: &Path, name: &str| read_secret(&d.join(SECRETS).join(name)).unwrap();
    for name in GENERATED {
        assert_ne!(secret(&a, name), secret(&b, name), "{name}");
    }
    let values: std::collections::BTreeSet<_> =
        GENERATED.iter().map(|name| secret(&a, name)).collect();
    assert_eq!(values.len(), GENERATED.len());
    let _ = (std::fs::remove_dir_all(&a), std::fs::remove_dir_all(&b));
}

#[test]
fn a_second_init_changes_nothing_and_a_replay_keeps_the_secrets() {
    let dir = dir("again");
    let installation = Installation::for_version("1.4.0");
    init(&dir, &installation, &Provided::default(), system_random).unwrap();
    let secrets_before: Vec<String> = GENERATED
        .iter()
        .map(|name| read_secret(&dir.join(SECRETS).join(name)).unwrap())
        .collect();
    let env_before = read(&dir, ".env");

    let other = Installation::for_version("2.0.0");
    let outcome = init(&dir, &other, &Provided::default(), system_random).unwrap();
    assert_eq!(outcome, vec![Outcome::Kept(dir.join(CONFIG))]);
    assert_eq!(read(&dir, ".env"), env_before);

    // A replay from donka.yaml writes the same files and keeps every secret.
    std::fs::remove_file(dir.join(".env")).unwrap();
    let replayed = Installation::read(&dir).unwrap();
    assert_eq!(replayed, installation);
    apply(&dir, &replayed, &Provided::default(), |_| {
        Err("no new secret".into())
    })
    .unwrap();
    assert_eq!(read(&dir, ".env"), env_before);
    let secrets_after: Vec<String> = GENERATED
        .iter()
        .map(|name| read_secret(&dir.join(SECRETS).join(name)).unwrap())
        .collect();
    assert_eq!(secrets_after, secrets_before);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failing_random_source_writes_no_secret() {
    let dir = dir("norandom");
    let failed = init(
        &dir,
        &Installation::for_version("1.4.0"),
        &Provided::default(),
        |_| Err("no entropy".into()),
    );
    assert_eq!(failed.unwrap_err(), "no entropy");
    assert!(!dir.join(SECRETS).join(POSTGRES_PASSWORD).exists());
    assert!(!dir.join(CONFIG).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_demo_turns_on_its_profile_and_its_connector_secret() {
    let plain = dir("plain");
    init(
        &plain,
        &Installation::for_version("1.4.0"),
        &Provided::default(),
        system_random,
    )
    .unwrap();
    let env = read(&plain, ".env");
    assert!(!env.contains("DONKA_DEMO") && !env.contains("COMPOSE_PROFILES"));
    assert!(!env.contains("DONKA_UPDATE_CHECK"));
    assert!(!read(&plain, "runtime.env").contains("DONKA_SECRET_"));

    let demo = dir("demo");
    let installation = Installation {
        role: Role::Demo,
        demo: true,
        offline: true,
        images: Images {
            registry: Some("registry.bank.example/donka".into()),
            ..Installation::for_version("1.4.0").images
        },
        ..Installation::for_version("1.4.0")
    };
    init(&demo, &installation, &Provided::default(), system_random).unwrap();
    let env = read(&demo, ".env");
    assert_eq!(value(&env, "COMPOSE_PROFILES"), "demo");
    assert_eq!(value(&env, "DONKA_UPDATE_CHECK"), "false");
    assert_eq!(value(&env, "DONKA_DEMO"), "true");
    assert_eq!(value(&env, "DONKA_REGISTRY"), "registry.bank.example/donka");
    let runtime = read(&demo, "runtime.env");
    assert_eq!(
        value(&runtime, "DONKA_SECRET_BUREAU_API_KEY_FILE"),
        "/run/secrets/connector_bureau_api_key"
    );
    let key = read_secret(&demo.join(SECRETS).join("connector_bureau_api_key")).unwrap();
    assert_eq!(key.len(), 48);
    assert!(!runtime.contains(&key));
}

#[test]
fn unattended_upgrades_are_refused_in_production() {
    let window = Window::parse("Sun 02:00-04:00").unwrap();
    let production = Installation {
        auto_upgrade: Some(window),
        ..Installation::for_version("1.4.0")
    };
    let refused = init(
        &dir("role-production"),
        &production,
        &Provided::default(),
        system_random,
    )
    .unwrap_err();
    assert!(refused.contains("never run in production"), "{refused}");

    let staging = dir("role-staging");
    let installation = Installation {
        role: Role::Staging,
        ..production.clone()
    };
    init(&staging, &installation, &Provided::default(), system_random).unwrap();
    let env = read(&staging, ".env");
    assert_eq!(value(&env, "DONKA_INSTALLATION_ROLE"), "staging");
    assert_eq!(value(&env, "COMPOSE_PROFILES"), "auto-upgrade");
    assert_eq!(value(&env, "DONKA_AUTO_UPGRADE_WINDOW"), "Sun 02:00-04:00");

    let demo = dir("role-demo");
    let installation = Installation {
        role: Role::Demo,
        demo: true,
        ..production
    };
    init(&demo, &installation, &Provided::default(), system_random).unwrap();
    assert_eq!(
        value(&read(&demo, ".env"), "COMPOSE_PROFILES"),
        "demo,auto-upgrade"
    );
}

#[test]
fn the_customers_services_read_their_secrets_from_files() {
    let dir = dir("services");
    let licence = dir.join("licence-from-donka.json");
    std::fs::write(&licence, "{\"format\":\"donka-licence/1\"}").unwrap();
    let installation = Installation {
        public_url: "https://donka.bank.example".into(),
        second_step: SecondStep::Privileged,
        storage: Storage::S3 {
            bucket: "releases".into(),
            region: "eu-west-1".into(),
            endpoint: Some("http://silo:9000".into()),
        },
        mail: Mail::Smtp {
            from: "Donka <donka@bank.example>".into(),
        },
        explanations: Explanations::Anthropic {
            url: "https://api.anthropic.com/v1/messages".into(),
            model: "claude-opus-5-5".into(),
        },
        licence: true,
        ..Installation::for_version("1.4.0")
    };
    // Without the operator's secrets, nothing usable is written and the missing file is named.
    let missing = init(&dir, &installation, &Provided::default(), system_random).unwrap_err();
    assert!(missing.contains("smtp_url is missing"), "{missing}");

    let provided = Provided {
        smtp_url: Some("smtp://ada:pw@mail.bank.example:587".into()),
        storage_access_key_id: Some("AKIA".into()),
        storage_secret_access_key: Some("s3-secret".into()),
        explain_api_key: Some("llm-key".into()),
        licence: Some(licence),
    };
    init(&dir, &installation, &provided, system_random).unwrap();
    let env = read(&dir, ".env");
    assert_eq!(value(&env, "DONKA_COOKIE_SECURE"), "true");
    assert_eq!(value(&env, "DONKA_MFA_REQUIRED"), "privileged");
    assert_eq!(value(&env, "DONKA_SMTP_URL_FILE"), "/run/secrets/smtp_url");
    assert_eq!(value(&env, "DONKA_STORAGE_URL"), "s3://releases");
    assert_eq!(value(&env, "DONKA_STORAGE_ENDPOINT"), "http://silo:9000");
    assert_eq!(
        value(&env, "DONKA_EXPLAIN_API_KEY_FILE"),
        "/run/secrets/explain_api_key"
    );
    assert_eq!(
        value(&env, "DONKA_LICENCE_FILE"),
        "/run/secrets/licence.json"
    );
    let runtime = read(&dir, "runtime.env");
    assert_eq!(value(&runtime, "PROVIDER__TYPE"), "S3");
    assert_eq!(value(&runtime, "PROVIDER__PREFIX"), "staging/");
    assert_eq!(
        value(&runtime, "AWS_SECRET_ACCESS_KEY_FILE"),
        "/run/secrets/storage_secret_access_key"
    );
    for secret in ["ada:pw", "AKIA", "s3-secret", "llm-key"] {
        for name in [".env", "runtime.env", CONFIG] {
            assert!(!read(&dir, name).contains(secret), "{name} holds {secret}");
        }
    }
    assert_eq!(
        read_secret(&dir.join(SECRETS).join(SMTP_URL)).unwrap(),
        "smtp://ada:pw@mail.bank.example:587"
    );
    assert!(read(&dir, &format!("{SECRETS}/{LICENCE}")).contains("donka-licence/1"));
}

#[test]
fn an_upgrade_changes_only_the_versions_in_donka_yaml() {
    let dir = dir("versions");
    init(
        &dir,
        &Installation::for_version("1.4.0"),
        &Provided::default(),
        system_random,
    )
    .unwrap();
    let yaml = read(&dir, CONFIG) + "# the operator's note\n";
    std::fs::write(dir.join(CONFIG), &yaml).unwrap();
    set_versions(&dir, "1.5.0", "1.3.0").unwrap();
    let after = read(&dir, CONFIG);
    assert!(
        after.contains("  studio: '1.5.0'") && after.contains("  runtime: '1.3.0'"),
        "{after}"
    );
    assert!(after.contains("# the operator's note"));
    assert_eq!(Installation::read(&dir).unwrap().images.studio, "1.5.0");
    assert_eq!(value(&read(&dir, ".env"), "DONKA_VERSION"), "1.5.0");
    assert_eq!(value(&read(&dir, ".env"), "DONKA_RUNTIME_VERSION"), "1.3.0");
}
