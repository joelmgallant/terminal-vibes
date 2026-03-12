use terminal_vibes::config::Config;

#[test]
fn test_gui_config_defaults() {
    let config = Config::default();
    assert_eq!(config.gui.width, 1280);
    assert_eq!(config.gui.height, 720);
    assert!(config.gui.vsync);
    assert!(!config.gui.fullscreen);
}

#[test]
fn test_gui_config_from_toml() {
    let toml_str = r#"
    [gui]
    width = 1920
    height = 1080
    vsync = false
    fullscreen = true
    "#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.gui.width, 1920);
    assert_eq!(config.gui.height, 1080);
    assert!(!config.gui.vsync);
    assert!(config.gui.fullscreen);
}
