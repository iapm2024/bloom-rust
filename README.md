# bloom-rust

> Nord-themed Terminal Cherry Blossom Screensaver written in Rust.

## Overview
`bloom-rust` is an ASCII cherry blossom tree animation and terminal screensaver with wind physics, dynamic altitude color shading, seasonal palettes, and twinkling night sky.

## Features
- 🌸 **Cherry Blossom Simulation**: Particles detach and gently drift with wind sway physics.
- 🌌 **Twinkling Starfield**: 5-tier Nord Frost twinkling star glow in night mode.
- 🌦️ **Weather FX**: Real-time environmental rain and cloud integration via `wttr.in` (enabled by default).
- 🎨 **Seasonal Modes**: Spring blossom, Summer greens, Autumn amber, Winter frost, or Auto (auto-detects hemisphere).
- 🌲 **Nord Theme**: Pure Nord Polar Night and Snow Storm background styling.
- 🔒 **Fully Portable**: Pure-Rust TLS (`rustls`) — no system OpenSSL or `pkg-config` needed.

## Prerequisites
- **Rust & Cargo**: Install via [rustup](https://rustup.rs/):
  ```bash
  curl https://sh.rustup.rs -sSf | sh
  ```
  No other system libraries are required.

## Installation
```bash
./install.sh
```
This compiles an optimized release binary and installs it to `~/.local/bin/bloom-rust` (and `~/.cargo/bin/bloom-rust`).

## Uninstallation
```bash
./install.sh --uninstall
```

## CLI Options
```
Usage: bloom-rust [OPTIONS]

Options:
  -s, --speed <SPEED>       Petal falling speed multiplier (0.1 - 10.0)
  -w, --sway <SWAY>         Wind sway amplitude multiplier
  -a, --art <FILE>          Custom ASCII art file path
      --stars               Enable twinkling starry background (default: on)
  -m, --mood <MOOD>         Theme mood: day | night (default: night)
      --season <SEASON>     Override season: spring | summer | autumn | winter | auto
      --hemisphere <HEMI>   Hemisphere: north | south | auto (default: auto)
      --no-weather          Disable real-time weather integration
      --city <CITY>         City for weather query
      --about               Show about page
  -h, --help                Print help
  -V, --version             Print version
```

## Controls
| Key | Action |
| :--- | :--- |
| `1` | Switch to **Spring** (Cherry Blossom Pink) |
| `2` | Switch to **Summer** (Aurora Green & Warm Amber) |
| `3` | Switch to **Autumn** (Aurora Crimson & Gold) |
| `4` | Switch to **Winter** (Frost Cyan & Pure Snow) |
| `m` / `M` | Toggle **Day / Night** mood palette |
| `a` / `?` | Toggle About & Shortcuts overlay |
| `q` / `Esc` | Quit screensaver |

## License
Licensed under GNU General Public License v3. Author: iapizarro.
