//! Content of the account emails, in the recipient's language.

use crate::Locale;
use chrono::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Invitation,
    PasswordReset,
    Notice(Notice),
}

/// Notices about the second sign-in step (DNK-59): no link, nothing to click.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Notice {
    SecondStepEnabled,
    SecondStepRemoved,
    SecondStepReset,
    RecoveryCodeUsed,
}

impl Kind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Invitation => "invitation",
            Self::PasswordReset => "password_reset",
            Self::Notice(Notice::SecondStepEnabled) => "second_step_enabled",
            Self::Notice(Notice::SecondStepRemoved) => "second_step_removed",
            Self::Notice(Notice::SecondStepReset) => "second_step_reset",
            Self::Notice(Notice::RecoveryCodeUsed) => "recovery_code_used",
        }
    }

    /// Invitations and resets carry a one-time link; notices do not.
    pub(crate) fn carries_link(self) -> bool {
        !matches!(self, Self::Notice(_))
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value {
            "invitation" => Some(Self::Invitation),
            "password_reset" => Some(Self::PasswordReset),
            "second_step_enabled" => Some(Self::Notice(Notice::SecondStepEnabled)),
            "second_step_removed" => Some(Self::Notice(Notice::SecondStepRemoved)),
            "second_step_reset" => Some(Self::Notice(Notice::SecondStepReset)),
            "recovery_code_used" => Some(Self::Notice(Notice::RecoveryCodeUsed)),
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
        (Kind::Notice(which), _) => notice(which, locale),
    }
}

/// A notice about the second sign-in step. It tells the person what changed and
/// what to do if it was not them; it never contains a link or a code.
fn notice(which: Notice, locale: Locale) -> Content {
    let (subject, what) = match (which, locale) {
        (Notice::SecondStepEnabled, Locale::En) => (
            "Two-step sign-in is on",
            "Two-step sign-in is now on for your Donka account. Each sign-in will ask for a \
             code from your authenticator app.",
        ),
        (Notice::SecondStepEnabled, Locale::Fr) => (
            "La connexion en deux étapes est activée",
            "La connexion en deux étapes est activée sur votre compte Donka. Chaque connexion \
             vous demandera un code de votre application d'authentification.",
        ),
        (Notice::SecondStepRemoved, Locale::En) => (
            "Two-step sign-in is off",
            "Two-step sign-in has been turned off for your Donka account. Signing in now asks \
             only for your password.",
        ),
        (Notice::SecondStepRemoved, Locale::Fr) => (
            "La connexion en deux étapes est désactivée",
            "La connexion en deux étapes a été désactivée sur votre compte Donka. La connexion \
             ne demande plus que votre mot de passe.",
        ),
        (Notice::SecondStepReset, Locale::En) => (
            "Your two-step sign-in was reset",
            "An administrator reset the two-step sign-in of your Donka account and signed you \
             out. Set it up again from your account, or at your next sign-in if your \
             organisation requires it.",
        ),
        (Notice::SecondStepReset, Locale::Fr) => (
            "Votre connexion en deux étapes a été réinitialisée",
            "Un administrateur a réinitialisé la connexion en deux étapes de votre compte Donka \
             et vous a déconnecté(e). Configurez-la à nouveau depuis votre compte, ou à votre \
             prochaine connexion si votre organisation l'exige.",
        ),
        (Notice::RecoveryCodeUsed, Locale::En) => (
            "A recovery code was used",
            "A recovery code was used to sign in to your Donka account. Each code works once; \
             generate new ones from your account if few are left.",
        ),
        (Notice::RecoveryCodeUsed, Locale::Fr) => (
            "Un code de secours a été utilisé",
            "Un code de secours a été utilisé pour vous connecter à votre compte Donka. Chaque \
             code ne fonctionne qu'une fois ; générez-en de nouveaux depuis votre compte s'il \
             en reste peu.",
        ),
    };
    let text = match locale {
        Locale::En => {
            format!("Hello,\n\n{what}\n\nIf this was not you, tell your Donka administrator now.\n")
        }
        Locale::Fr => format!(
            "Bonjour,\n\n{what}\n\nSi vous n'êtes pas à l'origine de ce changement, prévenez \
             immédiatement votre administrateur Donka.\n"
        ),
    };
    Content {
        subject: subject.into(),
        text,
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

    #[test]
    fn notices_carry_no_link_in_both_languages() {
        for kind in [
            Notice::SecondStepEnabled,
            Notice::SecondStepRemoved,
            Notice::SecondStepReset,
            Notice::RecoveryCodeUsed,
        ]
        .map(Kind::Notice)
        {
            assert!(!kind.carries_link());
            for locale in [Locale::En, Locale::Fr] {
                let content = render(kind, locale, "https://studio/x?token=t", Duration::hours(1));
                assert!(!content.text.contains("https://"));
                assert!(!content.subject.is_empty());
            }
            assert_eq!(Kind::parse(kind.as_str()), Some(kind));
        }
    }
}
