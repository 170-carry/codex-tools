use super::*;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("codex-catalog-{}", Uuid::new_v4()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
    fn bound_key(&self) -> Option<String> {
        bound_proxy_key_in_paths(
            &self.path("config.toml"),
            &self.path("auth.json"),
            &self.path("metadata.json"),
        )
        .unwrap()
    }
    fn managed_binding(&self) {
        fs::write(
            self.path("config.toml"),
            "model_provider = \"openai\"\nopenai_base_url = \"http://127.0.0.1:8787/v1\"\n",
        )
        .unwrap();
        fs::write(
            self.path("auth.json"),
            r#"{"auth_mode":"apikey","OPENAI_API_KEY":"test-key"}"#,
        )
        .unwrap();
        fs::write(self.path("metadata.json"), r#"{"config_existed":true,"auth_existed":true,"bound_base_url":"http://127.0.0.1:8787/v1","bound_at":1}"#).unwrap();
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn legacy_binding_gets_explicit_catalog_without_touching_cache_or_preferences() {
    let fixture = Fixture::new();
    fixture.managed_binding();
    fs::write(fixture.path("models_cache.json"), "existing native cache").unwrap();
    assert_eq!(fixture.bound_key().as_deref(), Some("test-key"));
    let catalog = serde_json::json!({"models": [{"slug": "gpt-6.1-sol"}]});
    let config_path = fixture.path("config.toml");
    sync_proxy_catalog_at(&config_path, &catalog).unwrap();
    let config = fs::read_to_string(&config_path).unwrap();
    let document = config.parse::<DocumentMut>().unwrap();
    assert_eq!(
        document["model_catalog_json"].as_str(),
        catalog_path(&config_path).to_str()
    );
    assert_eq!(document["model_provider"].as_str(), Some("openai"));
    assert_eq!(
        fs::read_to_string(fixture.path("models_cache.json")).unwrap(),
        "existing native cache"
    );
    let modified = fs::metadata(catalog_path(&config_path))
        .unwrap()
        .modified()
        .unwrap();
    sync_proxy_catalog_at(&config_path, &catalog).unwrap();
    assert_eq!(
        fs::metadata(catalog_path(&config_path))
            .unwrap()
            .modified()
            .unwrap(),
        modified
    );
    assert_eq!(fs::read_to_string(&config_path).unwrap(), config);
}

#[test]
fn sync_does_not_adopt_unmanaged_provider_or_catalog() {
    let fixture = Fixture::new();
    assert_eq!(fixture.bound_key(), None);
    fixture.managed_binding();
    let original = fs::read_to_string(fixture.path("config.toml")).unwrap();
    for config in [
        original.replace("8787", "9999"),
        original.replace("\"openai\"", "\"custom\""),
        format!("model_catalog_json = \"/user/catalog.json\"\n{original}"),
    ] {
        fs::write(fixture.path("config.toml"), config).unwrap();
        assert_eq!(fixture.bound_key(), None);
    }
}

#[test]
fn rebind_to_a_new_port_keeps_policy_sync_enabled() {
    let fixture = Fixture::new();
    fixture.managed_binding();
    let config = fs::read_to_string(fixture.path("config.toml"))
        .unwrap()
        .replace("8787", "8788");
    fs::write(fixture.path("config.toml"), config).unwrap();
    assert_eq!(fixture.bound_key(), None);
    record_bound_base_url(&fixture.path("metadata.json"), "http://127.0.0.1:8788/v1").unwrap();
    assert_eq!(fixture.bound_key().as_deref(), Some("test-key"));
    let metadata: CodexProxyBindingBackupMetadata =
        serde_json::from_str(&fs::read_to_string(fixture.path("metadata.json")).unwrap()).unwrap();
    assert!(metadata.config_existed && metadata.auth_existed);
    assert_eq!(metadata.bound_at, 1);
}

#[test]
fn account_switch_removes_only_managed_catalog() {
    for path in [
        "/tmp/codex-tools-models.json",
        r"C:\Users\test\.codex\codex-tools-models.json",
    ] {
        let mut document = DocumentMut::new();
        document["model_catalog_json"] = value(path);
        let raw = document.to_string();
        let chatgpt = config::build_chatgpt_profile_config(Some(&raw))
            .parse::<DocumentMut>()
            .unwrap();
        assert!(chatgpt.get("model_catalog_json").is_none());
        let relay =
            config::build_relay_profile_config(Some(&raw), "https://relay.invalid/v1", "gpt-6-sol")
                .parse::<DocumentMut>()
                .unwrap();
        assert!(relay.get("model_catalog_json").is_none());
    }
    let raw = "model_catalog_json = '/custom/catalog.json'\n";
    let chatgpt = config::build_chatgpt_profile_config(Some(raw))
        .parse::<DocumentMut>()
        .unwrap();
    assert_eq!(
        chatgpt["model_catalog_json"].as_str(),
        Some("/custom/catalog.json")
    );
}

#[test]
fn rebind_preserves_original_custom_catalog_backup_for_restore() {
    let fixture = Fixture::new();
    let config_path = fixture.path("config.toml");
    let original = "model_catalog_json = '/custom/catalog.json'\nmodel = 'gpt-6-sol'\n";
    fs::write(&config_path, original).unwrap();
    let backup = fixture.path("backup");
    ensure_codex_proxy_backup_in_dir(
        &config_path,
        &fixture.path("auth.json"),
        &backup,
        "http://127.0.0.1:8787/v1",
    )
    .unwrap();
    sync_proxy_catalog_at(&config_path, &serde_json::json!({"models":[]})).unwrap();
    ensure_codex_proxy_backup_in_dir(
        &config_path,
        &fixture.path("auth.json"),
        &backup,
        "http://127.0.0.1:8787/v1",
    )
    .unwrap();
    restore_backup_file(&backup.join("config.toml"), &config_path, true).unwrap();
    assert_eq!(fs::read_to_string(&config_path).unwrap(), original);
}
