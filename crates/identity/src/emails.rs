//! Content of the account emails, in the recipient's language.

use crate::Locale;
use chrono::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Invitation,
    PasswordReset,
}

impl Kind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Invitation => "invitation",
            Self::PasswordReset => "password_reset",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "invitation" => Some(Self::Invitation),
            "password_reset" => Some(Self::PasswordReset),
            _ => None,
        }
    }
}

pub(crate) struct Content {
    pub subject: String,
    pub text: String,
}

pub(crate) fn render(kind: Kind, locale: Locale, link: &str, valid_for: Duration) -> Content {
    let valid_for = describe(valid_for, locale);
    match (kind, locale) {
        (Kind::Invitation, Locale::En) => Content {
            subject: "You are invited to Donka".into(),
            text: format!(
                "Hello,\n\n\
                 You have been invited to Donka. Choose your password to activate your account:\n\n\
                 {link}\n\n\
                 The link works once and expires in {valid_for}. If it has expired, ask your \
                 administrator to invite you again.\n\n\
                 If you did not expect this invitation, you can ignore this email.\n"
            ),
        },
        (Kind::Invitation, Locale::Fr) => Content {
            subject: "Vous êtes invité(e) sur Donka".into(),
            text: format!(
                "Bonjour,\n\n\
                 Vous avez été invité(e) sur Donka. Choisissez votre mot de passe pour activer \
                 votre compte :\n\n\
                 {link}\n\n\
                 Le lien ne fonctionne qu'une fois et expire dans {valid_for}. S'il a expiré, \
                 demandez à votre administrateur de vous inviter à nouveau.\n\n\
                 Si vous n'attendiez pas cette invitation, ignorez cet e-mail.\n"
            ),
        },
        (Kind::PasswordReset, Locale::En) => Content {
            subject: "Reset your Donka password".into(),
            text: format!(
                "Hello,\n\n\
                 Someone asked to reset the password of your Donka account. To choose a new \
                 password, open this link:\n\n\
                 {link}\n\n\
                 The link works once and expires in {valid_for}. Setting a new password signs \
                 you out everywhere.\n\n\
                 If you did not ask for this, ignore this email: your password stays the same.\n"
            ),
        },
        (Kind::PasswordReset, Locale::Fr) => Content {
            subject: "Réinitialisez votre mot de passe Donka".into(),
            text: format!(
                "Bonjour,\n\n\
                 Une réinitialisation du mot de passe de votre compte Donka a été demandée. Pour \
                 choisir un nouveau mot de passe, ouvrez ce lien :\n\n\
                 {link}\n\n\
                 Le lien ne fonctionne qu'une fois et expire dans {valid_for}. Un nouveau mot de \
                 passe vous déconnecte de toutes vos sessions.\n\n\
                 Si vous n'êtes pas à l'origine de cette demande, ignorez cet e-mail : votre mot \
                 de passe reste inchangé.\n"
            ),
        },
    }
}

/// "72 hours", "30 minutes", "1 heure"… Whole hours when the duration allows.
fn describe(duration: Duration, locale: Locale) -> String {
    let minutes = duration.num_minutes().max(1);
    let (count, unit) = if minutes % 60 == 0 {
        (minutes / 60, Unit::Hour)
    } else {
        (minutes, Unit::Minute)
    };
    let word = match (unit, locale, count == 1) {
        (Unit::Hour, Locale::En, true) => "hour",
        (Unit::Hour, Locale::En, false) => "hours",
        (Unit::Hour, Locale::Fr, true) => "heure",
        (Unit::Hour, Locale::Fr, false) => "heures",
        (Unit::Minute, Locale::En, true) => "minute",
        (Unit::Minute, Locale::En, false) => "minutes",
        (Unit::Minute, Locale::Fr, true) => "minute",
        (Unit::Minute, Locale::Fr, false) => "minutes",
    };
    format!("{count} {word}")
}

#[derive(Clone, Copy)]
enum Unit {
    Hour,
    Minute,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_naturally() {
        assert_eq!(describe(Duration::hours(72), Locale::En), "72 hours");
        assert_eq!(describe(Duration::hours(1), Locale::Fr), "1 heure");
        assert_eq!(describe(Duration::minutes(30), Locale::Fr), "30 minutes");
        assert_eq!(describe(Duration::minutes(90), Locale::En), "90 minutes");
    }

    #[test]
    fn every_email_carries_the_link_in_both_languages() {
        for kind in [Kind::Invitation, Kind::PasswordReset] {
            for locale in [Locale::En, Locale::Fr] {
                let content = render(kind, locale, "https://studio/x?token=t", Duration::hours(1));
                assert!(content.text.contains("https://studio/x?token=t"));
                assert!(!content.subject.is_empty());
            }
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
    }
}
