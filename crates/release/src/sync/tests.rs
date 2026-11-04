use super::*;

#[test]
fn targets_read_as_pipelines_write_them() {
    let id = Uuid::new_v4();
    assert_eq!(Target::parse("main"), Ok(Target::Main));
    assert_eq!(
        Target::parse(&format!("commit:{id}")),
        Ok(Target::Release(id))
    );
    let v = SemVer {
        major: 1,
        minor: 4,
        patch: 0,
    };
    assert_eq!(Target::parse("release:1.4.0"), Ok(Target::Version(v)));
    assert_eq!(Target::parse("release:v1.4.0"), Ok(Target::Version(v)));
    assert_eq!(
        Target::parse("env:production"),
        Ok(Target::Environment(Environment::Production))
    );
    assert_eq!(Target::parse("branch:feature"), Err("UNSUPPORTED_TARGET"));
    for bad in [
        "",
        "latest",
        "commit:abc",
        "release:1.4",
        "release:1.4.0.1",
        "release:1.-4.0",
        "env:dev",
        "tag:x",
    ] {
        assert_eq!(Target::parse(bad), Err("INVALID_TARGET"), "{bad}");
    }
}
