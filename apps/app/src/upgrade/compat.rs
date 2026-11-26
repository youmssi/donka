//! What a release says about the installations it can upgrade (`apps/app/upgrade.json`, copied
//! to `/etc/donka/upgrade.json` in the image, so its signature covers it): the Studio versions it
//! upgrades from and the Runtime versions it works with. Edit the file in the release that breaks
//! compatibility; the release notes say the same in words.

use semver::{Version, VersionReq};
use serde::Deserialize;

/// Where the file is in Studio's image.
pub const PATH: &str = "/etc/donka/upgrade.json";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Compatibility {
    pub upgrades_from: VersionReq,
    pub runtime: VersionReq,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    Compatible,
    /// The release refuses this installation; the reason says which rule.
    Incompatible(String),
    /// A version is not a release (`edge`, `sha-…`): nothing to compare. Only staging and demo
    /// installations run those.
    Unchecked(String),
}

pub fn parse(json: &[u8]) -> Result<Compatibility, String> {
    serde_json::from_slice(json).map_err(|err| format!("{PATH} is not valid: {err}"))
}

impl Compatibility {
    /// Whether an installation running Studio `studio` and the Runtime `runtime` can move to
    /// this release.
    pub fn check(&self, studio: &str, runtime: &str) -> Verdict {
        let (Ok(studio_version), Ok(runtime_version)) = (release(studio), release(runtime)) else {
            return Verdict::Unchecked(format!(
                "Studio {studio} and Runtime {runtime} are not both release versions"
            ));
        };
        if !self.upgrades_from.matches(&studio_version) {
            return Verdict::Incompatible(format!(
                "this release upgrades Studio {}, not {studio}: upgrade through an intermediate \
                 release first",
                self.upgrades_from
            ));
        }
        if !self.runtime.matches(&runtime_version) {
            return Verdict::Incompatible(format!(
                "this release needs Runtime {}, not {runtime}: upgrade the Runtime too \
                 (--runtime-version)",
                self.runtime
            ));
        }
        Verdict::Compatible
    }
}

fn release(tag: &str) -> Result<Version, semver::Error> {
    Version::parse(tag.strip_prefix('v').unwrap_or(tag))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(from: &str, runtime: &str) -> Compatibility {
        parse(format!(r#"{{"upgradesFrom": "{from}", "runtime": "{runtime}"}}"#).as_bytes())
            .unwrap()
    }

    #[test]
    fn the_shipped_file_is_valid() {
        parse(include_bytes!("../../upgrade.json")).unwrap();
    }

    #[test]
    fn a_release_refuses_versions_its_file_excludes() {
        let rules = rules(">=1.4.0", ">=1.2.0");
        assert_eq!(rules.check("1.4.0", "1.2.0"), Verdict::Compatible);
        assert_eq!(rules.check("v1.5.2", "1.3.0"), Verdict::Compatible);
        assert!(
            matches!(rules.check("1.3.9", "1.2.0"), Verdict::Incompatible(why) if why.contains("intermediate"))
        );
        assert!(
            matches!(rules.check("1.4.0", "1.1.0"), Verdict::Incompatible(why) if why.contains("--runtime-version"))
        );
    }

    #[test]
    fn unreleased_versions_cannot_be_compared() {
        let rules = rules(">=1.0.0", ">=1.0.0");
        assert!(matches!(
            rules.check("edge", "1.0.0"),
            Verdict::Unchecked(_)
        ));
        assert!(matches!(
            rules.check("1.0.0", "sha-abc"),
            Verdict::Unchecked(_)
        ));
    }

    #[test]
    fn unknown_fields_are_refused() {
        assert!(parse(br#"{"upgradesFrom": ">=1", "runtime": ">=1", "from": "x"}"#).is_err());
    }
}
