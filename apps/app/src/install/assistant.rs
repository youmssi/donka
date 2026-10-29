//! The questions `donka-admin init` asks on a terminal (DNK-74), in English or French, in the
//! manner of `create-next-app`. Every question has a default: Enter keeps it. The answers
//! become `donka.yaml`; the secrets typed here go to `secrets/` only.

use super::{Explanations, Installation, Language, Mail, Provided, SecondStep, Storage};
use crate::upgrade::auto::{Role, Window};
use std::io::{BufRead as _, Write as _};
use std::path::PathBuf;

/// Where questions are asked and answered.
pub trait Prompt {
    /// The answer to `question`, `default` when it is left empty.
    fn ask(&mut self, question: &str, default: &str) -> Result<String, String>;
    fn say(&mut self, text: &str);
}

/// The operator's terminal.
pub struct Terminal;

impl Prompt for Terminal {
    fn ask(&mut self, question: &str, default: &str) -> Result<String, String> {
        let mut out = std::io::stdout();
        if default.is_empty() {
            print!("{question}: ");
        } else {
            print!("{question} [{default}]: ");
        }
        out.flush().map_err(|err| err.to_string())?;
        let mut line = String::new();
        let read = std::io::stdin()
            .lock()
            .read_line(&mut line)
            .map_err(|err| err.to_string())?;
        if read == 0 {
            return Err("the questions were interrupted: nothing was written".to_owned());
        }
        let answer = line.trim();
        Ok(if answer.is_empty() { default } else { answer }.to_owned())
    }

    fn say(&mut self, text: &str) {
        println!("{text}");
    }
}

/// The questions, in the operator's language, starting from `defaults` (the command line's
/// options already applied).
pub fn run(
    prompt: &mut impl Prompt,
    defaults: &Installation,
) -> Result<(Installation, Provided), String> {
    let mut i = defaults.clone();
    let mut provided = Provided::default();
    i.language = choose(
        prompt,
        "Language / Langue (en, fr)",
        i.language.as_str(),
        |answer| match answer {
            "en" => Some(Language::En),
            "fr" => Some(Language::Fr),
            _ => None,
        },
    )?;
    let t = Text::new(i.language);
    prompt.say(t.intro);
    i.public_url = ask_valid(prompt, t.public_url, &i.public_url, |url| {
        url.starts_with("http://") || url.starts_with("https://")
    })?;
    i.administrator = ask_valid(prompt, t.administrator, &i.administrator, |email| {
        email.contains('@')
    })?;
    let role_default = if i.demo { Role::Demo } else { i.role };
    i.role = choose(prompt, t.role, role_default.as_str(), |answer| {
        Role::parse(answer).ok()
    })?;
    i.demo = i.role == Role::Demo;
    i.second_step = choose(
        prompt,
        t.second_step,
        i.second_step.as_str(),
        |answer| match answer {
            "none" => Some(SecondStep::None),
            "privileged" => Some(SecondStep::Privileged),
            "all" => Some(SecondStep::All),
            _ => None,
        },
    )?;
    let storage = choose(prompt, t.storage, "disk", |answer| {
        matches!(answer, "disk" | "s3").then(|| answer.to_owned())
    })?;
    if storage == "s3" {
        let bucket = ask_valid(prompt, t.bucket, "donka-releases", |b| !b.is_empty())?;
        let region = prompt.ask(t.region, "us-east-1")?;
        let endpoint = prompt.ask(t.endpoint, "")?;
        provided.storage_access_key_id =
            Some(ask_valid(prompt, t.access_key, "", |k| !k.is_empty())?);
        provided.storage_secret_access_key =
            Some(ask_valid(prompt, t.secret_key, "", |k| !k.is_empty())?);
        i.storage = Storage::S3 {
            bucket,
            region,
            endpoint: (!endpoint.is_empty()).then_some(endpoint),
        };
    }
    let mail = choose(prompt, t.mail, "mailpit", |answer| {
        matches!(answer, "mailpit" | "smtp").then(|| answer.to_owned())
    })?;
    if mail == "smtp" {
        provided.smtp_url = Some(ask_valid(prompt, t.smtp_url, "", |url| {
            url.starts_with("smtp")
        })?);
        let from = prompt.ask(t.smtp_from, "Donka <donka@localhost>")?;
        i.mail = Mail::Smtp { from };
    }
    let model = choose(prompt, t.explanations, "off", |answer| {
        matches!(answer, "off" | "anthropic" | "openai").then(|| answer.to_owned())
    })?;
    if model != "off" {
        let (url, name) = if model == "anthropic" {
            ("https://api.anthropic.com/v1/messages", "claude-opus-5-5")
        } else {
            ("https://api.openai.com/v1/chat/completions", "gpt-5")
        };
        let url = prompt.ask(t.model_url, url)?;
        let model_name = prompt.ask(t.model_name, name)?;
        provided.explain_api_key = Some(ask_valid(prompt, t.model_key, "", |k| !k.is_empty())?);
        i.explanations = if model == "anthropic" {
            Explanations::Anthropic {
                url,
                model: model_name,
            }
        } else {
            Explanations::Openai {
                url,
                model: model_name,
            }
        };
    }
    let licence = ask_valid(prompt, t.licence, "", |path| {
        path.is_empty() || std::path::Path::new(path).is_file()
    })?;
    i.licence = !licence.is_empty();
    provided.licence = i.licence.then(|| PathBuf::from(licence));
    let online = choose(
        prompt,
        t.internet,
        if i.offline { "no" } else { "yes" },
        yes_no,
    )?;
    i.offline = !online;
    if i.role.upgrades_unattended() {
        let current = i.auto_upgrade.map(|w| w.to_string()).unwrap_or_default();
        i.auto_upgrade = choose(prompt, t.auto_upgrade, &current, |answer| {
            if answer.is_empty() {
                Some(None)
            } else {
                Window::parse(answer).ok().map(Some)
            }
        })?;
    } else {
        i.auto_upgrade = None;
    }
    if !choose(prompt, t.confirm, "yes", yes_no)? {
        return Err(t.cancelled.to_owned());
    }
    Ok((i, provided))
}

fn yes_no(answer: &str) -> Option<bool> {
    match answer {
        "yes" | "y" | "oui" | "o" => Some(true),
        "no" | "n" | "non" => Some(false),
        _ => None,
    }
}

/// Asks until an answer is one of the choices `parse` accepts.
fn choose<T>(
    prompt: &mut impl Prompt,
    question: &str,
    default: &str,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<T, String> {
    for _ in 0..5 {
        let answer = prompt.ask(question, default)?;
        if let Some(value) = parse(answer.trim()) {
            return Ok(value);
        }
    }
    Err(format!(
        "no valid answer to “{question}”: nothing was written"
    ))
}

fn ask_valid(
    prompt: &mut impl Prompt,
    question: &str,
    default: &str,
    valid: impl Fn(&str) -> bool,
) -> Result<String, String> {
    choose(prompt, question, default, |answer| {
        valid(answer).then(|| answer.to_owned())
    })
}

struct Text {
    intro: &'static str,
    public_url: &'static str,
    administrator: &'static str,
    role: &'static str,
    second_step: &'static str,
    storage: &'static str,
    bucket: &'static str,
    region: &'static str,
    endpoint: &'static str,
    access_key: &'static str,
    secret_key: &'static str,
    mail: &'static str,
    smtp_url: &'static str,
    smtp_from: &'static str,
    explanations: &'static str,
    model_url: &'static str,
    model_name: &'static str,
    model_key: &'static str,
    licence: &'static str,
    internet: &'static str,
    auto_upgrade: &'static str,
    confirm: &'static str,
    cancelled: &'static str,
}

impl Text {
    fn new(language: Language) -> Self {
        match language {
            Language::En => Self {
                intro: "Prepare this Donka installation. Press Enter to keep the value in brackets.",
                public_url: "Where people reach Studio (behind your HTTPS proxy)",
                administrator: "The first administrator's email",
                role: "What this installation is (production, staging, demo)",
                second_step: "Who signs in with a second step (none, privileged, all)",
                storage: "Where releases are published (disk on this server, s3)",
                bucket: "S3 bucket",
                region: "S3 region",
                endpoint: "S3-compatible server URL (empty for AWS)",
                access_key: "S3 access key id (saved in secrets/)",
                secret_key: "S3 secret access key (saved in secrets/)",
                mail: "How Studio sends email (mailpit catches it here, smtp)",
                smtp_url: "SMTP URL with credentials, e.g. smtp://user:password@mail.example:587?tls=required (saved in secrets/)",
                smtp_from: "Sender address",
                explanations: "Language model that explains decisions (off, anthropic, openai)",
                model_url: "Its endpoint URL",
                model_name: "Its model",
                model_key: "Its API key (saved in secrets/)",
                licence: "Donka's licence file, in this folder (empty: evaluation)",
                internet: "Does this server reach the internet? (yes, no)",
                auto_upgrade: "Unattended upgrades: weekly window, e.g. Sun 02:00-04:00 (empty: off)",
                confirm: "Write the installation? (yes, no)",
                cancelled: "Cancelled: nothing was written.",
            },
            Language::Fr => Self {
                intro: "Préparez cette installation de Donka. Entrée garde la valeur entre crochets.",
                public_url: "Adresse de Studio pour ses utilisateurs (derrière votre proxy HTTPS)",
                administrator: "Adresse e-mail du premier administrateur",
                role: "Rôle de cette installation (production, staging, demo)",
                second_step: "Qui se connecte avec une seconde étape (none, privileged, all)",
                storage: "Où les versions sont publiées (disk sur ce serveur, s3)",
                bucket: "Bucket S3",
                region: "Région S3",
                endpoint: "URL du serveur compatible S3 (vide pour AWS)",
                access_key: "Identifiant de clé d'accès S3 (enregistré dans secrets/)",
                secret_key: "Clé d'accès secrète S3 (enregistrée dans secrets/)",
                mail: "Envoi des e-mails (mailpit les retient ici, smtp)",
                smtp_url: "URL SMTP avec identifiants, p. ex. smtp://user:password@mail.example:587?tls=required (enregistrée dans secrets/)",
                smtp_from: "Adresse d'expéditeur",
                explanations: "Modèle de langage qui explique les décisions (off, anthropic, openai)",
                model_url: "URL de son point d'accès",
                model_name: "Son modèle",
                model_key: "Sa clé d'API (enregistrée dans secrets/)",
                licence: "Fichier de licence de Donka, dans ce dossier (vide : évaluation)",
                internet: "Ce serveur accède-t-il à Internet ? (yes, no)",
                auto_upgrade: "Mises à jour sans surveillance : fenêtre hebdomadaire, p. ex. Sun 02:00-04:00 (vide : désactivées)",
                confirm: "Écrire l'installation ? (yes, no)",
                cancelled: "Annulé : rien n'a été écrit.",
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    /// Answers from a script; an empty answer keeps the default.
    struct Script {
        answers: VecDeque<&'static str>,
        asked: Vec<String>,
    }

    impl Script {
        fn new(answers: &[&'static str]) -> Self {
            Self {
                answers: answers.iter().copied().collect(),
                asked: Vec::new(),
            }
        }
    }

    impl Prompt for Script {
        fn ask(&mut self, question: &str, default: &str) -> Result<String, String> {
            self.asked.push(question.to_owned());
            let answer = self.answers.pop_front().ok_or("out of answers")?;
            Ok(if answer.is_empty() { default } else { answer }.to_owned())
        }

        fn say(&mut self, _: &str) {}
    }

    #[test]
    fn enter_everywhere_keeps_every_default() {
        let defaults = Installation::for_version("1.5.0");
        // language, URL, admin, role, second step, storage, mail, model, licence, internet, confirm
        let mut script = Script::new(&[""; 11]);
        let (installation, provided) = run(&mut script, &defaults).unwrap();
        assert_eq!(installation, defaults);
        assert!(provided.smtp_url.is_none() && provided.explain_api_key.is_none());
        assert!(script.answers.is_empty(), "every default took one answer");
    }

    #[test]
    fn a_staging_installation_in_french_with_its_own_services() {
        let defaults = Installation::for_version("1.5.0");
        let mut script = Script::new(&[
            "fr",
            "https://donka.banque.example",
            "ada@banque.example",
            "staging",
            "privileged",
            "s3",
            "donka-releases",
            "",
            "http://silo:9000",
            "AKIA",
            "s3-secret",
            "smtp",
            "smtp://ada:pw@mail.banque.example:587",
            "Donka <donka@banque.example>",
            "openai",
            "http://llm.banque.example/v1/chat/completions",
            "",
            "llm-key",
            "",
            "non",
            "Sat 22:00-23:30",
            "oui",
        ]);
        let (i, provided) = run(&mut script, &defaults).unwrap();
        assert!(
            script.asked[1].starts_with("Adresse de Studio"),
            "{:?}",
            script.asked
        );
        assert_eq!(i.language, Language::Fr);
        assert_eq!(i.role, Role::Staging);
        assert_eq!(i.second_step, SecondStep::Privileged);
        assert_eq!(
            i.storage,
            Storage::S3 {
                bucket: "donka-releases".into(),
                region: "us-east-1".into(),
                endpoint: Some("http://silo:9000".into())
            }
        );
        assert_eq!(
            i.mail,
            Mail::Smtp {
                from: "Donka <donka@banque.example>".into()
            }
        );
        assert_eq!(
            i.explanations,
            Explanations::Openai {
                url: "http://llm.banque.example/v1/chat/completions".into(),
                model: "gpt-5".into()
            }
        );
        assert!(i.offline && !i.licence);
        assert_eq!(i.auto_upgrade.unwrap().to_string(), "Sat 22:00-23:30");
        assert_eq!(
            provided.storage_secret_access_key.as_deref(),
            Some("s3-secret")
        );
        assert_eq!(
            provided.smtp_url.as_deref(),
            Some("smtp://ada:pw@mail.banque.example:587")
        );
        assert_eq!(provided.explain_api_key.as_deref(), Some("llm-key"));
    }

    #[test]
    fn an_invalid_answer_is_asked_again() {
        let defaults = Installation::for_version("1.5.0");
        let mut script = Script::new(&[
            "",
            "donka.example",
            "https://donka.example",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
            "",
        ]);
        let (installation, _) = run(&mut script, &defaults).unwrap();
        assert_eq!(installation.public_url, "https://donka.example");
        assert_eq!(script.asked[1], script.asked[2]);
    }

    #[test]
    fn saying_no_at_the_end_writes_nothing() {
        let defaults = Installation::for_version("1.5.0");
        let mut answers = [""; 11];
        answers[10] = "no";
        let refused = run(&mut Script::new(&answers), &defaults).unwrap_err();
        assert!(refused.contains("nothing was written"), "{refused}");
    }
}
