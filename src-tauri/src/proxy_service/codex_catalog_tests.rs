use super::*;

fn slugs(catalog: &Value) -> Vec<&str> {
    catalog["models"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|model| model["visibility"] == "list" && model["supported_in_api"] == true)
        .map(|model| model["slug"].as_str().unwrap())
        .collect()
}

#[test]
fn native_catalog_exposes_sol_6_1_with_full_agent_metadata() {
    let catalog = catalog_for_key(&AppSettings::default(), &ApiProxyKey::default());
    assert_eq!(slugs(&catalog).len(), 8);
    assert!(!slugs(&catalog).contains(&"gpt-image-2"));
    let model = catalog["models"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["slug"] == "gpt-6.1-sol")
        .unwrap();
    assert_eq!(model["visibility"], "list");
    assert_eq!(model["supported_in_api"], true);
    assert_eq!(model["use_responses_lite"], true);
    assert_eq!(model["default_reasoning_level"], "low");
    assert_eq!(model["context_window"], 272000);
    assert!(model["base_instructions"].as_str().unwrap().len() > 1000);
    assert_eq!(
        model["base_instructions"],
        model["model_messages"]["instructions_template"]
    );
    for entry in catalog["models"].as_array().unwrap() {
        assert!(super::super::MODELS.contains(&entry["slug"].as_str().unwrap()));
    }
}

#[test]
fn native_catalog_intersects_switches_and_key_whitelist() {
    let key = ApiProxyKey {
        allowed_models: vec!["gpt6.1-sol".into(), "gpt-6-sol".into()],
        ..ApiProxyKey::default()
    };
    let settings = AppSettings {
        api_proxy_disabled_models: vec!["gpt-6-sol".into()],
        ..AppSettings::default()
    };
    assert_eq!(
        slugs(&catalog_for_key(&settings, &key)),
        vec!["gpt-6.1-sol"]
    );
    let settings = AppSettings {
        api_proxy_disabled_models: vec!["gpt6.1-sol".into()],
        ..AppSettings::default()
    };
    assert_eq!(slugs(&catalog_for_key(&settings, &key)), vec!["gpt-6-sol"]);
    let old_key = ApiProxyKey {
        allowed_models: vec!["gpt-6-sol".into()],
        ..ApiProxyKey::default()
    };
    assert_eq!(
        slugs(&catalog_for_key(&AppSettings::default(), &old_key)),
        vec!["gpt-6-sol"]
    );
}

#[test]
fn disabled_key_and_image_only_key_have_no_native_models() {
    let disabled = AppSettings {
        api_proxy_disabled_models: super::super::MODELS
            .iter()
            .map(|model| (*model).into())
            .collect(),
        ..AppSettings::default()
    };
    assert!(slugs(&catalog_for_key(&disabled, &ApiProxyKey::default())).is_empty());
    for key in [
        ApiProxyKey {
            enabled: false,
            ..ApiProxyKey::default()
        },
        ApiProxyKey {
            allowed_models: vec!["gpt-image-2".into()],
            ..ApiProxyKey::default()
        },
    ] {
        let catalog = catalog_for_key(&AppSettings::default(), &key);
        assert!(slugs(&catalog).is_empty());
        assert!(!catalog["models"].as_array().unwrap().is_empty());
    }
}
