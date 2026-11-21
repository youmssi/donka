use super::*;

#[test]
fn versions_count_up_from_1_0_0() {
    assert_eq!(SemVer::next(None, Bump::Patch), SemVer::FIRST);
    let v = SemVer {
        major: 1,
        minor: 4,
        patch: 2,
    };
    assert_eq!(SemVer::next(Some(v), Bump::Patch).to_string(), "1.4.3");
    assert_eq!(SemVer::next(Some(v), Bump::Minor).to_string(), "1.5.0");
    assert_eq!(SemVer::next(Some(v), Bump::Major).to_string(), "2.0.0");
}

#[test]
fn retries_back_off_up_to_an_hour() {
    assert_eq!(retry_delay(1), Duration::seconds(30));
    assert_eq!(retry_delay(2), Duration::seconds(60));
    assert_eq!(retry_delay(20), Duration::seconds(3600));
}

#[test]
fn environments_read_and_write() {
    for environment in Environment::ALL {
        assert_eq!(Environment::parse(environment.as_str()), Some(environment));
    }
    assert_eq!(Environment::parse("dev"), None);
}
