//! The two languages Donka writes in.
//!
//! Emails, alerts and documents are written for a person, not for a screen, so the module that
//! writes them needs to know the language without depending on the module that holds people.
//! A reader's locale (DNK-5) converts into this; everything written for them uses it.

/// The language a text is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    En,
    Fr,
}

impl Language {
    /// The segment the web app serves this language under, and the suffix of its documents.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::En => "en",
            Self::Fr => "fr",
        }
    }
}
