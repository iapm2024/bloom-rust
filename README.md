# bloom-rust

> Nord-themed Terminal Cherry Blossom Screensaver written in Rust.

## Overview
`bloom-rust` is an ASCII cherry blossom tree animation and terminal screensaver featuring organic canopy sway, leeward aerodynamic swirl vortices, 3D dual-layer parallax depth, rotational petal tumbling, organic terrain physics, dynamic altitude shading, real-time meteorological integration, and a pure Nord Polar Night aesthetic.

## Features
- **Organic Canopy Sway & Branch Flutter**: The tree structure bends and flexes harmonically with ambient breezes and gusts, with outer blossom tips fluttering while the trunk remains firmly grounded.
- **Leeward Swirl Vortices & Branch Bouncing**: Canopy drafting creates localized aerodynamic vortices that curve falling petals in spiral arcs, while inner branch collisions deflect descending petals.
- **Dual-Layer Parallax Simulation**: Foreground and background petal planes drift independently around the canopy for authentic 3D spatial depth.
- **Aerodynamic Rotational Tumbling**: Petals rotate dynamically with angular momentum, transitioning between broadside and edge-on silhouettes as they drift.
- **Contoured Organic Ground & Petal Mounds**: Undulating terrain baseline with grass tufts, accumulating petal drifts, and gust-induced ground rustle.
- **Meteorological FX & Puddle Reflections**: Real-time environmental rain, cloud, snow, and fog with animated puddle reflections and water ripple dynamics.
- **Twinkling Starfield**: 5-tier Nord Frost twinkling star glow in night mode.
- **Seasonal Modes**: Spring blossom, Summer greens, Autumn amber, Winter frost, or Auto (auto-detects hemisphere via GNOME/system location).
- **Nord Color Palette**: Pure Nord Polar Night, Frost, Snow Storm, and Aurora styling with smooth radial foliage glow.
- **Fully Portable**: Pure-Rust TLS (`rustls`) -- no system OpenSSL or `pkg-config` needed.

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
      --no-stars            Disable twinkling starry background
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
