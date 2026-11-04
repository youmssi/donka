use super::*;

#[test]
fn the_catalogue_holds_every_pack_of_the_repository() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs");
    let catalogue = Catalogue::load(&dir).unwrap();
    let keys: Vec<&str> = catalogue
        .packs()
        .iter()
        .map(|pack| pack.manifest.key.as_str())
        .collect();
    assert!(keys.windows(2).all(|pair| pair[0] < pair[1]), "{keys:?}");
    assert!(keys.contains(&"retail-credit"));
    assert!(catalogue.get("retail-credit").is_ok());
    assert!(matches!(catalogue.get("nope"), Err(PackError::NotFound)));
    assert!(Catalogue::load(&dir.join("missing")).is_err());
}

/// Each decision's input contract matches what its rules read (DNK-37).
#[test]
fn every_contract_of_the_catalogue_matches_its_rules() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs");
    for pack in Catalogue::load(&dir).unwrap().packs() {
        for (key, graph) in &pack.decisions {
            let schema = donka_engine::contract::input_schema(graph).unwrap();
            if let Some(schema) = schema {
                let warnings = donka_engine::contract::check(&pack.decisions, key, &schema);
                assert!(warnings.is_empty(), "{key}: {warnings:?}");
            }
        }
    }
}
