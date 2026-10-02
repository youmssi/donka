//! Content of the approval emails, in the recipient's language.

use crate::approval::Language;

pub(crate) struct Content {
    pub subject: String,
    pub body: String,
}

/// Tells an owner that a release waits for their approval before production.
pub(crate) fn approval_requested(
    language: Language,
    project: &str,
    version: &str,
    link: &str,
) -> Content {
    match language {
        Language::En => Content {
            subject: format!("{project}: release {version} waits for your approval"),
            body: format!(
                "Hello,\n\n\
                 Release {version} of {project} is live on staging and waits for an approval \
                 before it goes to production. Review what changes, the test results and the \
                 release notes, then approve or reject it:\n\n\
                 {link}\n\n\
                 Any owner of the project who neither made the release nor asked for it can \
                 decide; the first decision counts.\n"
            ),
        },
        Language::Fr => Content {
            subject: format!("{project} : la release {version} attend votre approbation"),
            body: format!(
                "Bonjour,\n\n\
                 La release {version} de {project} est en service en staging et attend une \
                 approbation avant de passer en production. Consultez les changements, les \
                 résultats des tests et les notes de la release, puis approuvez-la ou \
                 refusez-la :\n\n\
                 {link}\n\n\
                 Tout propriétaire du projet qui n'a ni créé la release ni demandé son passage \
                 peut décider ; la première décision compte.\n"
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_the_release_and_links_to_the_review() {
        for language in [Language::En, Language::Fr] {
            let email = approval_requested(language, "Crédit PME", "1.2.0", "https://x/a");
            assert!(email.subject.contains("1.2.0") && email.subject.contains("Crédit PME"));
            assert!(email.body.contains("https://x/a"));
        }
    }
}
