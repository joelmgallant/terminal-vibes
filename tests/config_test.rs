use terminal_vibes::config::Config;

#[test]
fn test_default_config_has_sensible_values() {
    let config = Config::default();
    assert_eq!(config.audio.fft_size, 2048);
    assert!((config.audio.smoothing - 0.7).abs() < f64::EPSILON);
    assert_eq!(config.audio.buffer_size, 4096);
    assert_eq!(config.display.fps, 30);
    assert!(config.display.show_status_bar);
    assert_eq!(config.keybindings.quit, "q");
    assert_eq!(config.keybindings.next_mode, "Tab");
}

#[test]
fn test_parse_partial_toml_uses_defaults_for_missing() {
    let toml_str = r#"
[audio]
fft_size = 4096

[display]
fps = 60
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.audio.fft_size, 4096);
    assert_eq!(config.display.fps, 60);
    // defaults for unspecified fields
    assert!((config.audio.smoothing - 0.7).abs() < f64::EPSILON);
    assert!(config.display.show_status_bar);
    assert_eq!(config.keybindings.quit, "q");
}

#[test]
fn test_parse_full_toml_roundtrip() {
    let toml_str = r#"
[audio]
fft_size = 1024
smoothing = 0.5
buffer_size = 8192

[display]
fps = 60
color_mode = "256"
show_status_bar = false

[keybindings]
next_mode = "n"
prev_mode = "p"
quit = "Escape"
toggle_status = "h"
increase_sensitivity = "="
decrease_sensitivity = "_"
"#;
    let config: Config = toml::from_str(toml_str).unwrap();
    assert_eq!(config.audio.fft_size, 1024);
    assert!((config.audio.smoothing - 0.5).abs() < f64::EPSILON);
    assert_eq!(config.audio.buffer_size, 8192);
    assert_eq!(config.display.fps, 60);
    assert_eq!(config.display.color_mode, "256");
    assert!(!config.display.show_status_bar);
    assert_eq!(config.keybindings.quit, "Escape");
}

#[test]
fn test_config_file_path_respects_default() {
    // When no XDG override, should use ~/.config/terminal-vibes/config.toml
    let path = Config::default_path();
    let path_str = path.to_string_lossy();
    assert!(path_str.contains("terminal-vibes"));
    assert!(path_str.ends_with("config.toml"));
}
