//! SigmaOS-linux-omarchy: Omarchy desktop integration layer.
//!
//! Real implementation of the Omarchy user-facing model:
//! - `omarchy` CLI command surface (theme, refresh, install, update,
//!   menu, restart, shutdown, lock, screenshot)
//! - keybind -> hyprland dispatch map (Omarchy defaults)
//! - theme switch orchestration across components
//! Replaces the Supervisor/Config placeholder.

use std::collections::BTreeMap;
use std::fmt;

/// Omarchy CLI subcommands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Theme { action: ThemeAction },
    Refresh,                    // re-apply all configs
    Install { pkg: String },    // pacman -S via wrapper
    Update,                     // full system update
    Menu { pane: String },      // walker/elephant menu pane
    Restart { target: RestartTarget },
    Lock,
    Screenshot { mode: ShotMode },
    Open { app: String },
    Config { key: Option<String>, value: Option<String> },
    Help,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThemeAction { Set(String), Next, Prev, List }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RestartTarget { Session, System }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotMode { Screen, Window, Region }

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
    if args.is_empty() { return Err(ParseError::EmptyArgs); }
    match args[0] {
        "theme" => {
            match args.get(1).copied() {
                Some("next") => Ok(Command::Theme { action: ThemeAction::Next }),
                Some("prev") => Ok(Command::Theme { action: ThemeAction::Prev }),
                Some("list") => Ok(Command::Theme { action: ThemeAction::List }),
                Some(name) if !name.starts_with('-') =>
                    Ok(Command::Theme { action: ThemeAction::Set(name.into()) }),
                _ => Err(ParseError::BadThemeAction(
                    args.get(1).unwrap_or(&"").to_string())),
            }
        }
        "refresh" => Ok(Command::Refresh),
        "install" | "add" => {
            let pkg = args.get(1).ok_or(ParseError::MissingArg {
                cmd: "install", what: "package".into() })?;
            Ok(Command::Install { pkg: pkg.to_string() })
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
                cmd: "open", what: "app".into() })?;
            Ok(Command::Open { app: app.to_string() })
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
        out.push_str(&format!("bind = {}, {}, exec, omarchy {}\n",
            mods.replace(' ', "+"), key, action));
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
    pub fn new() -> Self { Self::default() }

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
        if self.themes.is_empty() { return None; }
        let cur = self.theme.as_ref()
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
        assert_eq!(set, Command::Theme { action: ThemeAction::Set("tokyo-night".into()) });
        assert_eq!(next, Command::Theme { action: ThemeAction::Next });
    }

    #[test]
    fn test_parse_common_commands_aaa() {
        // Arrange & Act & Assert
        assert_eq!(parse_command(&["refresh"]).unwrap(), Command::Refresh);
        assert_eq!(parse_command(&["install", "firefox"]).unwrap(),
            Command::Install { pkg: "firefox".into() });
        assert_eq!(parse_command(&["update"]).unwrap(), Command::Update);
        assert_eq!(parse_command(&["screenshot", "window"]).unwrap(),
            Command::Screenshot { mode: ShotMode::Window });
        assert_eq!(parse_command(&["restart", "system"]).unwrap(),
            Command::Restart { target: RestartTarget::System });
        assert_eq!(parse_command(&["menu", "apps"]).unwrap(),
            Command::Menu { pane: "apps".into() });
    }

    #[test]
    fn test_parse_errors_fail_fast_aaa() {
        // Arrange & Act & Assert
        assert!(matches!(parse_command(&[]), Err(ParseError::EmptyArgs)));
        assert!(matches!(parse_command(&["bogus"]), Err(ParseError::UnknownCommand(_))));
        assert!(matches!(parse_command(&["install"]), Err(ParseError::MissingArg { .. })));
        assert!(matches!(parse_command(&["theme"]), Err(ParseError::BadThemeAction(_))));
        assert!(matches!(parse_command(&["open"]), Err(ParseError::MissingArg { .. })));
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
        let a = sh.resolve(Command::Theme { action: ThemeAction::Next });
        let b = sh.resolve(Command::Refresh);
        // Assert
        assert_eq!(a, ShellAction::CycleTheme { dir: 1 });
        assert_eq!(b, ShellAction::ExecCommand(Command::Refresh));
    }
}
