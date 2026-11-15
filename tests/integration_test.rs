use sigmaos_linux_omarchy::{
    AudioControl, BatteryInfo, BatteryStatus, BrightnessControl, CaptureEngine, CaptureTarget,
    Clipboard, DebugLogger, FontEntry, FontManager, LogLevel, OmarchyConfig, ThemeManager,
    ThemePreset, UpdateManager, UpdatePackage,
};

#[test]
fn test_audio_volume_control_aaa() {
    // Arrange
    let mut audio = AudioControl::new();
    // Act
    let res = audio.set_volume(75);
    // Assert
    assert!(res.is_ok());
    assert_eq!(audio.effective_volume(), 75);
}

#[test]
fn test_audio_volume_overflow_fails_fast_aaa() {
    // Arrange
    let mut audio = AudioControl::new();
    // Act
    let res = audio.set_volume(150);
    // Assert
    assert!(res.is_err());
}

#[test]
fn test_audio_mute_toggle_aaa() {
    // Arrange
    let mut audio = AudioControl::new();
    audio.set_volume(80).unwrap();
    // Act
    audio.toggle_mute();
    let muted_vol = audio.effective_volume();
    audio.toggle_mute();
    let unmuted_vol = audio.effective_volume();
    // Assert
    assert_eq!(muted_vol, 0);
    assert_eq!(unmuted_vol, 80);
}

#[test]
fn test_brightness_increase_decrease_aaa() {
    // Arrange
    let mut b = BrightnessControl::new(50);
    // Act
    b.increase(20);
    let after_inc = b.level;
    b.decrease(30);
    let after_dec = b.level;
    // Assert
    assert_eq!(after_inc, 70);
    assert_eq!(after_dec, 40);
}

#[test]
fn test_capture_build_command_aaa() {
    // Arrange
    let engine = CaptureEngine::new();
    // Act
    let cmd = engine.build_command(&CaptureTarget::Screen, "/tmp/shot.png");
    // Assert
    assert!(cmd.contains("screen"));
    assert!(cmd.contains("/tmp/shot.png"));
}

#[test]
fn test_clipboard_copy_paste_aaa() {
    // Arrange
    let mut clip = Clipboard::new();
    // Act
    clip.copy("hello world");
    // Assert
    assert_eq!(clip.paste(), Some("hello world"));
    assert_eq!(clip.history_len(), 1);
}

#[test]
fn test_theme_apply_aaa() {
    // Arrange
    let mut mgr = ThemeManager::new();
    // Act
    mgr.apply(ThemePreset::TokyoNight);
    // Assert
    assert_eq!(mgr.active_name(), "tokyo-night");
}

#[test]
fn test_config_set_get_aaa() {
    // Arrange
    let mut cfg = OmarchyConfig::new();
    // Act
    cfg.set("bar.position", "top");
    // Assert
    assert_eq!(cfg.get("bar.position"), Some("top"));
    assert_eq!(cfg.get("missing"), None);
}

#[test]
fn test_battery_critical_check_aaa() {
    // Arrange
    let batt = BatteryInfo::new(5, BatteryStatus::Discharging);
    let full = BatteryInfo::new(100, BatteryStatus::Full);
    // Act & Assert
    assert!(batt.is_critical());
    assert!(full.is_full());
}

#[test]
fn test_font_manager_monospace_filter_aaa() {
    // Arrange
    let mut mgr = FontManager::new();
    mgr.register(FontEntry {
        name: "JetBrains Mono".into(),
        family: "JetBrainsMono".into(),
        is_monospace: true,
        size: 12,
    });
    mgr.register(FontEntry {
        name: "Inter".into(),
        family: "Inter".into(),
        is_monospace: false,
        size: 11,
    });
    // Act
    let mono = mgr.monospace_fonts();
    // Assert
    assert_eq!(mono.len(), 1);
    assert_eq!(mono[0].name, "JetBrains Mono");
}

#[test]
fn test_update_manager_apply_aaa() {
    // Arrange
    let mut mgr = UpdateManager::new();
    mgr.add_pending(UpdatePackage {
        name: "hyprland".into(),
        current_version: "0.40.0".into(),
        new_version: "0.41.0".into(),
    });
    // Act
    let result = mgr.apply_update("hyprland").unwrap();
    // Assert
    assert!(result.contains("0.41.0"));
    assert_eq!(mgr.pending_count(), 0);
}

#[test]
fn test_debug_logger_level_filter_aaa() {
    // Arrange
    let mut logger = DebugLogger::new(LogLevel::Debug);
    logger.log(LogLevel::Error, "disk full");
    logger.log(LogLevel::Info, "started");
    // Act
    let errors = logger.entries_at_level(&LogLevel::Error);
    // Assert
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0], "disk full");
}
