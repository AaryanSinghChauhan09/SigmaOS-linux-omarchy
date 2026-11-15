# SigmaOS-linux-omarchy

> **Omarchy Linux integration layer** — CLI command surface, keybindings, theme orchestration.

Implements the `omarchy` command model: `theme`, `refresh`, `install`, `update`, `menu`,
`restart`, `lock`, `screenshot`, `open`, and `config` subcommands.

## Quickstart

```bash
cargo test   # run all 5+ deterministic AAA unit tests
cargo check  # verify compilation
```

## API Surface

```rust
use sigmaos_linux_omarchy::{parse_command, Command, ThemeAction};

let cmd = parse_command(&["theme", "set", "nord"])?;
// => Command::Theme { action: ThemeAction::Set("nord".into()) }

let cmd = parse_command(&["refresh"])?;
// => Command::Refresh
```

## Component Role

This crate is a component of the [SigmaOS](https://github.com/AaryanSinghChauhan09/SigmaOS) ecosystem.  
See the [GitHub Wiki](https://github.com/AaryanSinghChauhan09/SigmaOS/wiki) for full documentation.

## License

MIT
