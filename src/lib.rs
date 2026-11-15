//! SigmaOS-linux-omarchy: Omarchy desktop integration layer.
//!
//! Real implementation of the Omarchy user-facing model:
//! - `omarchy` CLI command surface (theme, refresh, install, update,
//!   menu, restart, shutdown, lock, screenshot)
//! - keybind -> hyprland dispatch map (Omarchy defaults)
//! - theme switch orchestration across components
//!
//! Replaces the Supervisor/Config placeholder.

use std::fmt;

/// Omarchy CLI subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Theme {
        action: ThemeAction,
    },
    Refresh, // re-apply all configs
    Install {
        pkg: String,
    }, // pacman -S via wrapper
    Update,  // full system update
    Menu {
        pane: String,
    }, // walker/elephant menu pane
    Restart {
        target: RestartTarget,
    },
    Lock,
    Screenshot {
        mode: ShotMode,
    },
    Open {
        app: String,
    },
    Config {
        key: Option<String>,
        value: Option<String>,
    },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeAction {
    Set(String),
    Next,
    Prev,
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartTarget {
    Session,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotMode {
    Screen,
    Window,
    Region,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    EmptyArgs,
    UnknownCommand(String),
    MissingArg { cmd: &'static str, what: String },
    BadThemeAction(String),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyArgs => write!(f, "no command given"),
            Self::UnknownCommand(c) => write!(f, "unknown command {:?}", c),
            Self::MissingArg { cmd, what } => write!(f, "{} requires {}", cmd, what),
            Self::BadThemeAction(a) => write!(f, "bad theme action {:?}", a),
        }
    }
}

impl std::error::Error for ParseError {}

/// Parse `omarchy <args>` argv (excluding argv[0]).
pub fn parse_command(args: &[&str]) -> Result<Command, ParseError> {
    if args.is_empty() {
        return Err(ParseError::EmptyArgs);
    }
    match args[0] {
        "theme" => match args.get(1).copied() {
            Some("next") => Ok(Command::Theme {
                action: ThemeAction::Next,
            }),
            Some("prev") => Ok(Command::Theme {
                action: ThemeAction::Prev,
            }),
            Some("list") => Ok(Command::Theme {
                action: ThemeAction::List,
            }),
            Some(name) if !name.starts_with('-') => Ok(Command::Theme {
                action: ThemeAction::Set(name.into()),
            }),
            _ => Err(ParseError::BadThemeAction(
                args.get(1).unwrap_or(&"").to_string(),
            )),
        },
        "refresh" => Ok(Command::Refresh),
        "install" | "add" => {
            let pkg = args.get(1).ok_or(ParseError::MissingArg {
                cmd: "install",
                what: "package".into(),
            })?;
            Ok(Command::Install {
                pkg: pkg.to_string(),
            })
        }
        "update" | "upgrade" => Ok(Command::Update),
        "menu" => Ok(Command::Menu {
            pane: args.get(1).unwrap_or(&"root").to_string(),
        }),
        "restart" => Ok(Command::Restart {
            target: match args.get(1).copied() {
                Some("system") => RestartTarget::System,
                _ => RestartTarget::Session,
            },
        }),
        "lock" => Ok(Command::Lock),
        "screenshot" | "shot" => Ok(Command::Screenshot {
            mode: match args.get(1).copied() {
                Some("window") => ShotMode::Window,
                Some("region") | Some("area") => ShotMode::Region,
                _ => ShotMode::Screen,
            },
        }),
        "open" | "launch" => {
            let app = args.get(1).ok_or(ParseError::MissingArg {
                cmd: "open",
                what: "app".into(),
            })?;
            Ok(Command::Open {
                app: app.to_string(),
            })
        }
        "config" | "set" => Ok(Command::Config {
            key: args.get(1).map(|s| s.to_string()),
            value: args.get(2).map(|s| s.to_string()),
        }),
        "help" | "--help" | "-h" => Ok(Command::Help),
        other => Err(ParseError::UnknownCommand(other.into())),
    }
}

/// Omarchy default keybinds: mods+key -> dispatch command.
/// (SUPER = modm, SHIFT, ALT, CTRL)
pub fn default_keybinds() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("SUPER SHIFT", "Return", "terminal"),
        ("SUPER SHIFT", "B", "browser"),
        ("SUPER SHIFT", "F", "file-manager"),
        ("SUPER SHIFT", "Space", "app-launcher"),
        ("SUPER SHIFT", "Q", "close-window"),
        ("SUPER", "Print", "screenshot-screen"),
        ("SUPER SHIFT", "Print", "screenshot-window"),
        ("SUPER CTRL", "T", "theme-menu"),
        ("SUPER CTRL", "R", "refresh-config"),
        ("SUPER", "Escape", "lock-screen"),
        ("SUPER CTRL", "Delete", "power-menu"),
        ("SUPER", "1", "workspace-1"),
        ("SUPER", "2", "workspace-2"),
        ("SUPER", "3", "workspace-3"),
        ("SUPER", "Tab", "workspace-next"),
        ("SUPER SHIFT", "Tab", "workspace-prev"),
    ]
}

/// Render keybinds to hyprland.conf `bind` lines.
pub fn render_binds() -> String {
    let mut out = String::new();
    for (mods, key, action) in default_keybinds() {
        out.push_str(&format!(
            "bind = {}, {}, exec, omarchy {}\n",
            mods.replace(' ', "+"),
            key,
            action
        ));
    }
    out
}

/// The integration facade: maps parsed commands to component actions.
#[derive(Debug, Default)]
pub struct OmarchyShell {
    /// currently applied theme name
    pub theme: Option<String>,
    /// ordered list of known theme names for next/prev
    pub themes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellAction {
    ExecCommand(Command),
    SetTheme { name: String },
    CycleTheme { dir: i8 },
    ListThemes(Vec<String>),
    Unknown,
}

impl OmarchyShell {
    pub fn new() -> Self {
        Self::default()
    }

    /// Resolve a CLI command to a shell action (theme commands become
    /// theme actions, everything else stays a command to dispatch).
    pub fn resolve(&self, cmd: Command) -> ShellAction {
        match cmd {
            Command::Theme { action } => match action {
                ThemeAction::Set(name) => ShellAction::SetTheme { name },
                ThemeAction::Next => ShellAction::CycleTheme { dir: 1 },
                ThemeAction::Prev => ShellAction::CycleTheme { dir: -1 },
                ThemeAction::List => ShellAction::ListThemes(self.themes.clone()),
            },
            other => ShellAction::ExecCommand(other),
        }
    }

    /// Compute next theme name given current index.
    pub fn cycle_theme(&self, dir: i8) -> Option<String> {
        if self.themes.is_empty() {
            return None;
        }
        let cur = self
            .theme
            .as_ref()
            .and_then(|t| self.themes.iter().position(|x| x == t))
            .unwrap_or(0);
        let n = self.themes.len() as i8;
        let next = ((cur as i8 + dir).rem_euclid(n)) as usize;
        Some(self.themes[next].clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_theme_set_and_cycle_aaa() {
        // Arrange & Act
        let set = parse_command(&["theme", "tokyo-night"]).unwrap();
        let next = parse_command(&["theme", "next"]).unwrap();
        // Assert
        assert_eq!(
            set,
            Command::Theme {
                action: ThemeAction::Set("tokyo-night".into())
            }
        );
        assert_eq!(
            next,
            Command::Theme {
                action: ThemeAction::Next
            }
        );
    }

    #[test]
    fn test_parse_common_commands_aaa() {
        // Arrange & Act & Assert
        assert_eq!(parse_command(&["refresh"]).unwrap(), Command::Refresh);
        assert_eq!(
            parse_command(&["install", "firefox"]).unwrap(),
            Command::Install {
                pkg: "firefox".into()
            }
        );
        assert_eq!(parse_command(&["update"]).unwrap(), Command::Update);
        assert_eq!(
            parse_command(&["screenshot", "window"]).unwrap(),
            Command::Screenshot {
                mode: ShotMode::Window
            }
        );
        assert_eq!(
            parse_command(&["restart", "system"]).unwrap(),
            Command::Restart {
                target: RestartTarget::System
            }
        );
        assert_eq!(
            parse_command(&["menu", "apps"]).unwrap(),
            Command::Menu {
                pane: "apps".into()
            }
        );
    }

    #[test]
    fn test_parse_errors_fail_fast_aaa() {
        // Arrange & Act & Assert
        assert!(matches!(parse_command(&[]), Err(ParseError::EmptyArgs)));
        assert!(matches!(
            parse_command(&["bogus"]),
            Err(ParseError::UnknownCommand(_))
        ));
        assert!(matches!(
            parse_command(&["install"]),
            Err(ParseError::MissingArg { .. })
        ));
        assert!(matches!(
            parse_command(&["theme"]),
            Err(ParseError::BadThemeAction(_))
        ));
        assert!(matches!(
            parse_command(&["open"]),
            Err(ParseError::MissingArg { .. })
        ));
    }

    #[test]
    fn test_default_keybinds_render_aaa() {
        // Arrange & Act
        let binds = render_binds();
        // Assert
        assert!(binds.contains("SUPER+SHIFT, Return"));
        assert!(binds.contains("exec, omarchy terminal"));
        assert!(binds.contains("workspace-1"));
        assert!(default_keybinds().len() >= 10);
    }

    #[test]
    fn test_theme_cycle_wraps_aaa() {
        // Arrange
        let mut sh = OmarchyShell::new();
        sh.themes = vec!["nord".into(), "catppuccin".into(), "tokyo".into()];
        sh.theme = Some("tokyo".into());
        // Act & Assert: next from tokyo wraps to nord
        assert_eq!(sh.cycle_theme(1), Some("nord".into()));
        assert_eq!(sh.cycle_theme(-1), Some("catppuccin".into()));
        // empty themes → None
        let empty = OmarchyShell::new();
        assert_eq!(empty.cycle_theme(1), None);
    }

    #[test]
    fn test_resolve_routes_theme_vs_exec_aaa() {
        // Arrange
        let mut sh = OmarchyShell::new();
        sh.themes = vec!["a".into(), "b".into()];
        // Act
        let a = sh.resolve(Command::Theme {
            action: ThemeAction::Next,
        });
        let b = sh.resolve(Command::Refresh);
        // Assert
        assert_eq!(a, ShellAction::CycleTheme { dir: 1 });
        assert_eq!(b, ShellAction::ExecCommand(Command::Refresh));
    }
}

// ============================================================
// === AUDIO ===
// ============================================================

pub struct AudioControl {
    pub volume: u8,
    pub muted: bool,
}

impl AudioControl {
    pub fn new() -> Self {
        Self {
            volume: 50,
            muted: false,
        }
    }

    pub fn set_volume(&mut self, vol: u8) -> Result<(), String> {
        if vol > 100 {
            return Err(format!("Volume {} exceeds max 100", vol));
        }
        self.volume = vol;
        Ok(())
    }

    pub fn toggle_mute(&mut self) {
        self.muted = !self.muted;
    }

    pub fn effective_volume(&self) -> u8 {
        if self.muted {
            0
        } else {
            self.volume
        }
    }
}

impl Default for AudioControl {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === BRIGHTNESS ===
// ============================================================

pub struct BrightnessControl {
    pub level: u8,
}

impl BrightnessControl {
    pub fn new(level: u8) -> Self {
        Self { level }
    }

    pub fn set(&mut self, level: u8) -> Result<(), String> {
        if level > 100 {
            return Err(format!("Level {} exceeds 100", level));
        }
        self.level = level;
        Ok(())
    }

    pub fn increase(&mut self, delta: u8) {
        self.level = self.level.saturating_add(delta).min(100);
    }

    pub fn decrease(&mut self, delta: u8) {
        self.level = self.level.saturating_sub(delta);
    }
}

// ============================================================
// === CAPTURE ===
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum CaptureTarget {
    Screen,
    Region,
    Window,
    ActiveWindow,
}

pub struct CaptureEngine {
    pub last_path: Option<String>,
}

impl CaptureEngine {
    pub fn new() -> Self {
        Self { last_path: None }
    }

    pub fn build_command(&self, target: &CaptureTarget, output: &str) -> String {
        let mode = match target {
            CaptureTarget::Screen => "screen",
            CaptureTarget::Region => "region",
            CaptureTarget::Window => "window",
            CaptureTarget::ActiveWindow => "active",
        };
        format!("grim -{} {}", mode, output)
    }

    pub fn set_last_path(&mut self, path: &str) {
        self.last_path = Some(path.to_string());
    }

    pub fn has_capture(&self) -> bool {
        self.last_path.is_some()
    }
}

impl Default for CaptureEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === CLIPBOARD ===
// ============================================================

pub struct Clipboard {
    stack: Vec<String>,
}

impl Clipboard {
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    pub fn copy(&mut self, text: &str) {
        self.stack.push(text.to_string());
    }

    pub fn paste(&self) -> Option<&str> {
        self.stack.last().map(|s| s.as_str())
    }

    pub fn history(&self) -> &[String] {
        &self.stack
    }

    pub fn clear(&mut self) {
        self.stack.clear();
    }

    pub fn history_len(&self) -> usize {
        self.stack.len()
    }
}

impl Default for Clipboard {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === THEME (new structs) ===
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum ThemePreset {
    CatppuccinMocha,
    TokyoNight,
    Nord,
    GruvboxDark,
    CyberSlate,
}

impl ThemePreset {
    pub fn name(&self) -> &str {
        match self {
            Self::CatppuccinMocha => "catppuccin-mocha",
            Self::TokyoNight => "tokyo-night",
            Self::Nord => "nord",
            Self::GruvboxDark => "gruvbox-dark",
            Self::CyberSlate => "cyber-slate",
        }
    }

    pub fn background_color(&self) -> &str {
        match self {
            Self::CatppuccinMocha => "#1e1e2e",
            Self::TokyoNight => "#1a1b26",
            Self::Nord => "#2e3440",
            Self::GruvboxDark => "#282828",
            Self::CyberSlate => "#0d1117",
        }
    }

    pub fn all() -> Vec<ThemePreset> {
        vec![
            Self::CatppuccinMocha,
            Self::TokyoNight,
            Self::Nord,
            Self::GruvboxDark,
            Self::CyberSlate,
        ]
    }
}

pub struct ThemeManager {
    pub active: ThemePreset,
}

impl ThemeManager {
    pub fn new() -> Self {
        Self {
            active: ThemePreset::CatppuccinMocha,
        }
    }

    pub fn apply(&mut self, preset: ThemePreset) {
        self.active = preset;
    }

    pub fn active_name(&self) -> &str {
        self.active.name()
    }

    pub fn list_themes() -> Vec<&'static str> {
        vec![
            "catppuccin-mocha",
            "tokyo-night",
            "nord",
            "gruvbox-dark",
            "cyber-slate",
        ]
    }
}

impl Default for ThemeManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === CONFIG ===
// ============================================================

pub struct OmarchyConfig {
    entries: std::collections::HashMap<String, String>,
}

impl OmarchyConfig {
    pub fn new() -> Self {
        Self {
            entries: std::collections::HashMap::new(),
        }
    }

    pub fn set(&mut self, key: &str, value: &str) {
        self.entries.insert(key.to_string(), value.to_string());
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.entries.get(key).map(|s| s.as_str())
    }

    pub fn keys(&self) -> Vec<&str> {
        self.entries.keys().map(|s| s.as_str()).collect()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl Default for OmarchyConfig {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === BATTERY ===
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum BatteryStatus {
    Charging,
    Discharging,
    Full,
    Unknown,
}

pub struct BatteryInfo {
    pub level: u8,
    pub status: BatteryStatus,
    pub time_remaining_min: Option<u32>,
}

impl BatteryInfo {
    pub fn new(level: u8, status: BatteryStatus) -> Self {
        Self {
            level,
            status,
            time_remaining_min: None,
        }
    }

    pub fn is_critical(&self) -> bool {
        self.level < 10 && self.status == BatteryStatus::Discharging
    }

    pub fn is_full(&self) -> bool {
        self.level >= 100 || self.status == BatteryStatus::Full
    }

    pub fn set_time_remaining(&mut self, mins: u32) {
        self.time_remaining_min = Some(mins);
    }
}

// ============================================================
// === FONT ===
// ============================================================

#[derive(Debug, Clone)]
pub struct FontEntry {
    pub name: String,
    pub family: String,
    pub is_monospace: bool,
    pub size: u32,
}

pub struct FontManager {
    fonts: Vec<FontEntry>,
}

impl FontManager {
    pub fn new() -> Self {
        Self { fonts: Vec::new() }
    }

    pub fn register(&mut self, entry: FontEntry) {
        self.fonts.push(entry);
    }

    pub fn monospace_fonts(&self) -> Vec<&FontEntry> {
        self.fonts.iter().filter(|f| f.is_monospace).collect()
    }

    pub fn find(&self, name: &str) -> Option<&FontEntry> {
        self.fonts.iter().find(|f| f.name == name)
    }

    pub fn count(&self) -> usize {
        self.fonts.len()
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === UPDATE ===
// ============================================================

#[derive(Debug, Clone)]
pub struct UpdatePackage {
    pub name: String,
    pub current_version: String,
    pub new_version: String,
}

pub struct UpdateManager {
    pending: Vec<UpdatePackage>,
}

impl UpdateManager {
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    pub fn add_pending(&mut self, pkg: UpdatePackage) {
        self.pending.push(pkg);
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }

    pub fn apply_update(&mut self, name: &str) -> Result<String, String> {
        if let Some(pos) = self.pending.iter().position(|p| p.name == name) {
            let pkg = self.pending.remove(pos);
            Ok(format!(
                "Updated {} from {} to {}",
                pkg.name, pkg.current_version, pkg.new_version
            ))
        } else {
            Err(format!("Package {} not in pending list", name))
        }
    }

    pub fn apply_all(&mut self) -> Vec<String> {
        let results: Vec<String> = self
            .pending
            .iter()
            .map(|p| format!("Updated {} to {}", p.name, p.new_version))
            .collect();
        self.pending.clear();
        results
    }
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === DEBUG ===
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

pub struct DebugLogger {
    entries: Vec<(LogLevel, String)>,
    pub level: LogLevel,
}

impl DebugLogger {
    pub fn new(level: LogLevel) -> Self {
        Self {
            entries: Vec::new(),
            level,
        }
    }

    pub fn log(&mut self, level: LogLevel, msg: &str) {
        self.entries.push((level, msg.to_string()));
    }

    pub fn entries_at_level(&self, level: &LogLevel) -> Vec<&str> {
        self.entries
            .iter()
            .filter(|(l, _)| l == level)
            .map(|(_, m)| m.as_str())
            .collect()
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }
}

// ============================================================
// === LOCAL AI CATALOG (inspired by omacom/omarchy-local-ai) ===
// ============================================================

#[derive(Debug, Clone, PartialEq)]
pub struct AiModel {
    pub id: String,
    pub name: String,
    pub context_window: u32,
    pub vram_gb: u32,
}

pub struct AiCatalog {
    models: Vec<AiModel>,
}

impl AiCatalog {
    pub fn new() -> Self {
        Self {
            models: vec![
                AiModel {
                    id: "deepseek-coder-v2".into(),
                    name: "DeepSeek Coder V2".into(),
                    context_window: 32768,
                    vram_gb: 12,
                },
                AiModel {
                    id: "hermes-3-8b".into(),
                    name: "Nous Hermes 3 8B".into(),
                    context_window: 16384,
                    vram_gb: 8,
                },
                AiModel {
                    id: "qwen2.5-coder-7b".into(),
                    name: "Qwen 2.5 Coder 7B".into(),
                    context_window: 32768,
                    vram_gb: 6,
                },
            ],
        }
    }

    pub fn list_models(&self) -> &[AiModel] {
        &self.models
    }

    pub fn find(&self, id: &str) -> Option<&AiModel> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn count(&self) -> usize {
        self.models.len()
    }
}

impl Default for AiCatalog {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === PORT FORWARD TUNNEL (inspired by omacom/omarchy-port-forward-plugin) ===
// ============================================================

#[derive(Debug, Clone)]
pub struct SshTunnel {
    pub name: String,
    pub local_port: u16,
    pub remote_host: String,
    pub remote_port: u16,
    pub is_active: bool,
}

pub struct TunnelManager {
    tunnels: Vec<SshTunnel>,
}

impl TunnelManager {
    pub fn new() -> Self {
        Self {
            tunnels: Vec::new(),
        }
    }

    pub fn add(&mut self, tunnel: SshTunnel) {
        self.tunnels.push(tunnel);
    }

    pub fn toggle(&mut self, name: &str) -> Result<bool, String> {
        if let Some(t) = self.tunnels.iter_mut().find(|t| t.name == name) {
            t.is_active = !t.is_active;
            Ok(t.is_active)
        } else {
            Err(format!("Tunnel '{}' not found", name))
        }
    }

    pub fn active_tunnels(&self) -> Vec<&SshTunnel> {
        self.tunnels.iter().filter(|t| t.is_active).collect()
    }

    pub fn count(&self) -> usize {
        self.tunnels.len()
    }
}

impl Default for TunnelManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================
// === PLUGIN MARKETPLACE (inspired by omacom/omarchy-plugin-marketplace) ===
// ============================================================

#[derive(Debug, Clone)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub author: String,
    pub installed: bool,
}

pub struct PluginMarketplace {
    plugins: Vec<PluginInfo>,
}

impl PluginMarketplace {
    pub fn new() -> Self {
        Self {
            plugins: vec![
                PluginInfo {
                    id: "theme-sync".into(),
                    name: "Theme Sync Broadcaster".into(),
                    author: "Omacom".into(),
                    installed: true,
                },
                PluginInfo {
                    id: "port-forward".into(),
                    name: "SSH Port Forward Drawer".into(),
                    author: "Omacom".into(),
                    installed: true,
                },
                PluginInfo {
                    id: "local-ai".into(),
                    name: "Local AI Catalog".into(),
                    author: "SigmaOS".into(),
                    installed: true,
                },
                PluginInfo {
                    id: "audio-tuner".into(),
                    name: "Speaker DSP Tuner".into(),
                    author: "Omacom".into(),
                    installed: false,
                },
            ],
        }
    }

    pub fn list(&self) -> &[PluginInfo] {
        &self.plugins
    }

    pub fn install(&mut self, id: &str) -> Result<(), String> {
        if let Some(p) = self.plugins.iter_mut().find(|p| p.id == id) {
            p.installed = true;
            Ok(())
        } else {
            Err(format!("Plugin '{}' not found", id))
        }
    }

    pub fn installed_plugins(&self) -> Vec<&PluginInfo> {
        self.plugins.iter().filter(|p| p.installed).collect()
    }
}

impl Default for PluginMarketplace {
    fn default() -> Self {
        Self::new()
    }
}
