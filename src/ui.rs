use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::config::{AppConfig, Hemisphere, Mood, Season};
use crate::physics::{LeafState, ParticleEngine};
use crate::stars::StarrySky;
use crate::weather::{WeatherFetcher, WeatherFxEngine, WeatherFxKind};
use chrono::{Datelike, Local};

#[inline]
fn set_cell(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16, ch: char, fg: Color, bg: Color) {
    if x < buf.area.width && y < buf.area.height {
        let cell = buf.get_mut(x, y);
        cell.set_char(ch);
        cell.set_fg(fg);
        cell.set_bg(bg);
    }
}

fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let vertical_margin = if area.height > height {
        (area.height - height) / 2
    } else {
        0
    };
    let horizontal_margin = if area.width > width {
        (area.width - width) / 2
    } else {
        0
    };

    Rect {
        x: area.x + horizontal_margin,
        y: area.y + vertical_margin,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

pub fn render_ui(
    f: &mut Frame,
    config: &AppConfig,
    weather: &WeatherFetcher,
    tree_grid: &[Vec<char>],
    particles: &ParticleEngine,
    stars: &StarrySky,
    weather_fx: &WeatherFxEngine,
    show_about: bool,
) {
    let area = f.size();
    if area.width < 10 || area.height < 5 {
        return;
    }

    let nord_bg = match config.mood {
        Mood::Night => Color::Rgb(46, 52, 64),   // Nord0 Polar Night (#2E3440)
        Mood::Day => Color::Rgb(229, 233, 240),  // Nord4 Snow Storm (#E5E9F0)
    };

    // 1. Fill solid screen background
    let bg_block = Block::default().style(Style::default().bg(nord_bg));
    f.render_widget(bg_block, area);

    let season = get_effective_season(config, weather);
    let blossom_colors = get_seasonal_colors(season);
    let buf = f.buffer_mut();

    // 2. Render Starry Sky if enabled with 5-tier Nord Frost twinkling glow
    if config.stars && config.mood == Mood::Night {
        for star in &stars.stars {
            if star.x < area.width && star.y < area.height.saturating_sub(2) {
                let val = star.phase.sin();
                let (ch, color) = if val > 0.75 {
                    ('*', Color::Rgb(236, 239, 244)) // Nord6 Pure Snow White (Peak twinkle)
                } else if val > 0.4 {
                    ('+', Color::Rgb(136, 192, 208)) // Nord8 Frost Cyan (Bright glow)
                } else if val > 0.0 {
                    ('.', Color::Rgb(143, 188, 187)) // Nord7 Frost Teal (Soft glow)
                } else if val > -0.5 {
                    ('.', Color::Rgb(129, 161, 193)) // Nord9 Glacial Blue (Dim star)
                } else {
                    ('.', Color::Rgb(94, 129, 172))  // Nord10 Deep Arctic Blue (Subtle background star)
                };

                set_cell(buf, star.x, star.y, ch, color, nord_bg);
            }
        }
    }

    // 2.5 Render Weather Particle FX Overlays
    for wp in &weather_fx.particles {
        let px = wp.x.round() as u16;
        let py = wp.y.round() as u16;

        if px < area.width && py < area.height.saturating_sub(1) {
            let color = match wp.kind {
                WeatherFxKind::RainDrop => match config.mood {
                    Mood::Night => Color::Rgb(129, 161, 193), // Nord9 Glacial Blue (#81A1C1)
                    Mood::Day => Color::Rgb(94, 129, 172),   // Nord10 Deep Arctic Blue (#5E81AC)
                },
                WeatherFxKind::RainSplash => Color::Rgb(136, 192, 208), // Nord8 Frost Cyan (#88C0D0)
                WeatherFxKind::SnowFlake => match wp.phase.sin() > 0.0 {
                    true => Color::Rgb(236, 239, 244),  // Nord6 Pure Snow (#ECEFF4)
                    false => Color::Rgb(216, 222, 233), // Nord4 Snow Storm (#D8DEE9)
                },
                WeatherFxKind::FogWisp => match config.mood {
                    Mood::Night => Color::Rgb(76, 86, 106),  // Nord3 Polar Night Slate (#4C566A)
                    Mood::Day => Color::Rgb(216, 222, 233),  // Nord4 Snow Storm Mist
                },
                WeatherFxKind::CloudWisp => match config.mood {
                    Mood::Night => Color::Rgb(76, 86, 106),
                    Mood::Day => Color::Rgb(216, 222, 233),
                },
                WeatherFxKind::SunGlimmer => match config.mood {
                    Mood::Night => Color::Rgb(136, 192, 208), // Nord8 Aurora Cyan Glimmer
                    Mood::Day => Color::Rgb(235, 203, 139),   // Nord13 Amber Sunlight Glimmer
                },
            };

            set_cell(buf, px, py, wp.ch, color, nord_bg);
        }
    }

    // 3. Render Static Tree Matrix (Adaptive scaling: 1:1 if it fits, or proportional downsampling)
    let art_height = tree_grid.len();
    let art_width = tree_grid.iter().map(|r| r.len()).max().unwrap_or(80);
    let target_height = area.height.saturating_sub(1) as usize;

    if art_height <= target_height {
        // 1:1 scale rendering (fits completely!)
        let offset_x = (area.width as usize).saturating_sub(art_width) / 2;
        let offset_y = target_height - art_height;

        for (r, row) in tree_grid.iter().enumerate() {
            let py = (offset_y + r) as u16;
            if py >= area.height.saturating_sub(1) {
                break;
            }

            for (c, &ch) in row.iter().enumerate() {
                let px = (offset_x + c) as u16;
                if px >= area.width || ch == ' ' {
                    continue;
                }

                let color = get_tree_char_color(ch, r, c, art_height, art_width, season, config.mood);
                set_cell(buf, px, py, ch, color, nord_bg);
            }
        }
    } else {
        // Proportional Scaling to fit both top canopy and grounded root base inside small terminal height
        let step_y = if target_height > 1 {
            (art_height - 1) as f32 / (target_height - 1) as f32
        } else {
            1.0
        };
        let step_x = (art_width as f32 / area.width.max(1) as f32).max(1.0);

        let scaled_width = (art_width as f32 / step_x).ceil() as u16;
        let offset_x = (area.width.saturating_sub(scaled_width)) / 2;

        for by in 0..target_height {
            let sy = ((by as f32 * step_y).round() as usize).min(art_height.saturating_sub(1));
            if sy < tree_grid.len() {
                let row = &tree_grid[sy];
                let row_len = row.len();
                let scaled_cols = (row_len as f32 / step_x).ceil() as usize;

                for bx in 0..scaled_cols {
                    let sx = ((bx as f32 * step_x).round() as usize).min(row_len.saturating_sub(1));
                    let ch = row[sx];
                    if ch != ' ' {
                        let px = offset_x + bx as u16;
                        let py = by as u16;

                        if px < area.width && py < area.height.saturating_sub(1) {
                            let color = get_tree_char_color(ch, sy, sx, art_height, art_width, season, config.mood);
                            set_cell(buf, px, py, ch, color, nord_bg);
                        }
                    }
                }
            }
        }
    }

    let ground_row = area.height.saturating_sub(2);

    // 4. Render Settled Ground Blossom Carpet with Soft Dissolution Shading
    for s in &particles.settled {
        if s.x < area.width && s.y <= ground_row {
            let base_color = blossom_colors[s.color_idx % blossom_colors.len()];

            // Multi-stage fading gradient as petals rest and dissolve into earth
            let color = if s.alpha > 0.65 {
                base_color
            } else if s.alpha > 0.35 {
                match season {
                    Season::Spring | Season::Auto => Color::Rgb(180, 142, 173), // Nord15 Sakura Pink (#B48EAD)
                    Season::Summer => Color::Rgb(143, 188, 187),                // Nord7 Frost Teal (#8FBCBB)
                    Season::Autumn => Color::Rgb(208, 135, 112),                // Nord12 Aurora Orange (#D08770)
                    Season::Winter => Color::Rgb(136, 192, 208),                // Nord8 Frost Cyan (#88C0D0)
                }
            } else if s.alpha > 0.15 {
                match config.mood {
                    Mood::Night => Color::Rgb(94, 129, 172),                    // Nord10 Deep Arctic Blue (#5E81AC)
                    Mood::Day => Color::Rgb(129, 161, 193),                     // Nord9 Glacial Blue (#81A1C1)
                }
            } else {
                match config.mood {
                    Mood::Night => Color::Rgb(76, 86, 106),                     // Nord3 Polar Night Slate (#4C566A)
                    Mood::Day => Color::Rgb(67, 76, 94),                       // Nord2 Deep Slate (#434C5E)
                }
            };

            // Soften particle character as petal dissolves into the ground
            let ch = if s.alpha < 0.25 {
                '.'
            } else if s.alpha < 0.5 {
                '+'
            } else {
                s.ch
            };

            set_cell(buf, s.x, s.y, ch, color, nord_bg);
        }
    }

    // 5. Render Attached & Falling Blossom Petals with height-dependent Nord gradient shading
    for leaf in &particles.leaves {
        let mut draw_x = leaf.x;
        if leaf.state == LeafState::Attached {
            let gust_sway = particles.sway_amplitude + (particles.gust_intensity * 1.5);
            draw_x += (leaf.phase + particles.time * 2.2).sin() * (gust_sway * 0.4);
        }

        let px = draw_x.round() as u16;
        let py = leaf.y.round() as u16;

        if px < area.width && py <= ground_row {
            let base_color = blossom_colors[leaf.color_idx % blossom_colors.len()];
            
            // Dynamic altitude gradient shading as petals drift downward
            let color = if leaf.state == LeafState::Falling {
                let progress = (py as f32 / ground_row.max(1) as f32).clamp(0.0, 1.0);
                if progress > 0.85 {
                    // Settling near ground: Nord Snow/Gold ground glow
                    match season {
                        Season::Spring | Season::Auto => Color::Rgb(216, 222, 233), // Nord4 Snow Storm
                        Season::Summer => Color::Rgb(229, 233, 240), // Nord5 Snow
                        Season::Autumn => Color::Rgb(235, 203, 139), // Nord13 Amber
                        Season::Winter => Color::Rgb(236, 239, 244), // Nord6 Snow
                    }
                } else if progress > 0.45 {
                    // Mid-air drift transition
                    match season {
                        Season::Spring | Season::Auto => Color::Rgb(136, 192, 208), // Nord8 Frost Cyan
                        Season::Summer => Color::Rgb(143, 188, 187), // Nord7 Frost Teal
                        Season::Autumn => Color::Rgb(208, 135, 112), // Nord12 Coral Orange
                        Season::Winter => Color::Rgb(136, 192, 208), // Nord8 Frost Cyan
                    }
                } else {
                    base_color
                }
            } else {
                base_color
            };

            set_cell(buf, px, py, leaf.ch, color, nord_bg);
        }
    }

    // 5. Render Clean Status Bar Footer (bottom line)
    let now = Local::now();
    let date_str = now.format("%A %d/%m/%Y").to_string();
    let weather_str = weather.get_weather_info();
    let season_str = match season {
        Season::Spring => "Spring",
        Season::Summer => "Summer",
        Season::Autumn => "Autumn",
        Season::Winter => "Winter",
        Season::Auto => "Auto",
    };
    let status_line = format!("{}  │  {}  │  {}", date_str, weather_str, season_str);

    let footer_rect = Rect::new(0, area.height.saturating_sub(1), area.width, 1);
    let footer_p = Paragraph::new(status_line)
        .alignment(Alignment::Center)
        .style(Style::default().fg(Color::Rgb(129, 161, 193)).bg(nord_bg).add_modifier(Modifier::BOLD));
    f.render_widget(footer_p, footer_rect);

    // 6. Render About Modal Overlay if toggled
    if show_about {
        render_about_modal(f, area, config);
    }
}

#[inline]
fn lerp_color(c1: Color, c2: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    match (c1, c2) {
        (Color::Rgb(r1, g1, b1), Color::Rgb(r2, g2, b2)) => {
            let r = (r1 as f32 + (r2 as f32 - r1 as f32) * t).round() as u8;
            let g = (g1 as f32 + (g2 as f32 - g1 as f32) * t).round() as u8;
            let b = (b1 as f32 + (b2 as f32 - b1 as f32) * t).round() as u8;
            Color::Rgb(r, g, b)
        }
        _ => c2,
    }
}

fn get_tree_char_color(
    ch: char,
    r: usize,
    c: usize,
    total_rows: usize,
    total_cols: usize,
    season: Season,
    mood: Mood,
) -> Color {
    let is_trunk = r >= (total_rows * 60 / 100);
    let is_lower_branches = r >= (total_rows * 48 / 100) && !is_trunk;

    if is_trunk {
        // Distinct Nord Slate / Bark Palette for the main trunk with vertical grounding
        match ch {
            '%' | '#' | '@' => match mood {
                Mood::Night => Color::Rgb(76, 86, 106),   // Nord3 Polar Night Slate Bark (#4C566A)
                Mood::Day => Color::Rgb(67, 76, 94),     // Nord2 Deep Slate (#434C5E)
            },
            '*' | '+' | '=' => match mood {
                Mood::Night => Color::Rgb(94, 129, 172),  // Nord10 Deep Arctic Blue (#5E81AC)
                Mood::Day => Color::Rgb(76, 86, 106),    // Nord3 Slate Bark (#4C566A)
            },
            _ => match mood {
                Mood::Night => Color::Rgb(129, 161, 193), // Nord9 Glacial Blue Bark Highlight (#81A1C1)
                Mood::Day => Color::Rgb(143, 188, 187),   // Nord7 Frost Teal Bark Highlight (#8FBCBB)
            },
        }
    } else if is_lower_branches {
        // Transition zone between trunk and canopy
        match season {
            Season::Spring | Season::Auto => match ch {
                '%' | '#' | '@' => match mood {
                    Mood::Night => Color::Rgb(94, 129, 172),  // Nord10 Deep Arctic Blue
                    Mood::Day => Color::Rgb(129, 161, 193),   // Nord9 Glacial Blue
                },
                '*' | '+' | '=' => match mood {
                    Mood::Night => Color::Rgb(180, 142, 173), // Nord15 Nordic Sakura Pink
                    Mood::Day => Color::Rgb(143, 188, 187),   // Nord7 Frost Teal
                },
                _ => Color::Rgb(216, 222, 233),                // Nord4 Snow Storm
            },
            Season::Summer => match ch {
                '%' | '#' | '@' => Color::Rgb(163, 190, 140),  // Nord14 Aurora Green
                '*' | '+' | '=' => Color::Rgb(235, 203, 139),  // Nord13 Amber Gold
                _ => Color::Rgb(143, 188, 187),                // Nord7 Frost Teal
            },
            Season::Autumn => match ch {
                '%' | '#' | '@' => Color::Rgb(191, 97, 106),   // Nord11 Aurora Red
                '*' | '+' | '=' => Color::Rgb(208, 135, 112),  // Nord12 Aurora Orange
                _ => Color::Rgb(235, 203, 139),                // Nord13 Amber Gold
            },
            Season::Winter => match ch {
                '%' | '#' | '@' => Color::Rgb(94, 129, 172),   // Nord10 Deep Arctic Blue
                '*' | '+' | '=' => Color::Rgb(136, 192, 208),  // Nord8 Frost Cyan
                _ => Color::Rgb(236, 239, 244),                // Nord6 Pure Snow White
            },
        }
    } else {
        // Proposal 6: TrueColor 3D Radial Foliage Depth & Canopy Glow Model
        let center_r = (total_rows as f32 * 0.28).max(1.0);
        let center_c = (total_cols as f32 * 0.50).max(1.0);
        let radius_r = (total_rows as f32 * 0.26).max(1.0);
        let radius_c = (total_cols as f32 * 0.42).max(1.0);

        let dr = (r as f32 - center_r) / radius_r;
        let dc = (c as f32 - center_c) / radius_c;
        let radial_dist = (dr * dr + dc * dc).sqrt().clamp(0.0, 1.0);

        let char_weight = match ch {
            '@' | '%' | '#' => 0.0,
            '+' | '=' | '*' => 0.4,
            _ => 0.85,
        };

        // Combined depth factor: blends radial distance from center with character density
        let depth = (radial_dist * 0.65 + char_weight * 0.35).clamp(0.0, 1.0);

        // Seasonal (Core Shadow, Mid Body, Radiant Outer Glow) color stops
        let (core, mid, glow) = match season {
            Season::Spring | Season::Auto => match mood {
                Mood::Night => (
                    Color::Rgb(94, 129, 172),   // Nord10 Deep Arctic Plum Core (#5E81AC)
                    Color::Rgb(180, 142, 173),  // Nord15 Nordic Sakura Pink Mid (#B48EAD)
                    Color::Rgb(236, 239, 244),  // Nord6 Pure Radiant Snow Glow (#ECEFF4)
                ),
                Mood::Day => (
                    Color::Rgb(129, 161, 193),  // Nord9 Glacial Blue
                    Color::Rgb(180, 142, 173),  // Nord15 Sakura Pink
                    Color::Rgb(236, 239, 244),  // Nord6 Snow Storm
                ),
            },
            Season::Summer => match mood {
                Mood::Night => (
                    Color::Rgb(76, 86, 106),    // Nord3 Polar Night Slate Core
                    Color::Rgb(163, 190, 140),  // Nord14 Lush Aurora Green Mid
                    Color::Rgb(235, 203, 139),  // Nord13 Amber Sunlight Glow
                ),
                Mood::Day => (
                    Color::Rgb(94, 129, 172),   // Nord10 Arctic Blue
                    Color::Rgb(163, 190, 140),  // Nord14 Aurora Green
                    Color::Rgb(235, 203, 139),  // Nord13 Warm Gold Glow
                ),
            },
            Season::Autumn => match mood {
                Mood::Night => (
                    Color::Rgb(180, 142, 173),  // Nord15 Deep Wine Core
                    Color::Rgb(191, 97, 106),   // Nord11 Aurora Crimson Mid
                    Color::Rgb(235, 203, 139),  // Nord13 Golden Amber Glow
                ),
                Mood::Day => (
                    Color::Rgb(208, 135, 112),  // Nord12 Aurora Coral
                    Color::Rgb(191, 97, 106),   // Nord11 Aurora Red
                    Color::Rgb(235, 203, 139),  // Nord13 Amber Gold Glow
                ),
            },
            Season::Winter => match mood {
                Mood::Night => (
                    Color::Rgb(94, 129, 172),   // Nord10 Deep Arctic Shadow
                    Color::Rgb(136, 192, 208),  // Nord8 Frost Cyan Mid
                    Color::Rgb(236, 239, 244),  // Nord6 Pure Ice White Glow
                ),
                Mood::Day => (
                    Color::Rgb(129, 161, 193),  // Nord9 Glacial Blue
                    Color::Rgb(136, 192, 208),  // Nord8 Frost Cyan
                    Color::Rgb(236, 239, 244),  // Nord6 Pure Snow
                ),
            },
        };

        if depth < 0.5 {
            lerp_color(core, mid, depth * 2.0)
        } else {
            lerp_color(mid, glow, (depth - 0.5) * 2.0)
        }
    }
}

fn get_effective_season(config: &AppConfig, weather: &WeatherFetcher) -> Season {
    if config.season != Season::Auto {
        return config.season;
    }

    let now = Local::now();
    let month = now.month();
    let day = now.day();

    let is_south = match config.hemisphere {
        Hemisphere::South => true,
        Hemisphere::North => false,
        Hemisphere::Auto => weather.is_southern_hemisphere(),
    };

    if is_south {
        // Chilean Astronomical Season Calendar (Southern Hemisphere)
        // Verano:   Dec 21 – Mar 20
        // Otoño:    Mar 21 – Jun 20
        // Invierno: Jun 21 – Sep 20
        // Primavera: Sep 21 – Dec 20
        match (month, day) {
            (12, 21..=31) | (1, _) | (2, _) | (3, 1..=20) => Season::Summer,
            (3, 21..=31) | (4, _) | (5, _) | (6, 1..=20) => Season::Autumn,
            (6, 21..=31) | (7, _) | (8, _) | (9, 1..=20) => Season::Winter,
            _ => Season::Spring,
        }
    } else {
        // Northern Hemisphere
        match (month, day) {
            (12, 21..=31) | (1, _) | (2, _) | (3, 1..=20) => Season::Winter,
            (3, 21..=31) | (4, _) | (5, _) | (6, 1..=20) => Season::Spring,
            (6, 21..=31) | (7, _) | (8, _) | (9, 1..=20) => Season::Summer,
            _ => Season::Autumn,
        }
    }
}

fn get_seasonal_colors(season: Season) -> Vec<Color> {
    match season {
        Season::Spring => vec![
            Color::Rgb(180, 142, 173), // Nord15 Nordic Purple
            Color::Rgb(136, 192, 208), // Nord8 Frost Cyan
            Color::Rgb(236, 239, 244), // Nord6 Snow White
            Color::Rgb(143, 188, 187), // Nord7 Frost Teal
        ],
        Season::Summer => vec![
            Color::Rgb(163, 190, 140), // Nord14 Aurora Green
            Color::Rgb(235, 203, 139), // Nord13 Warm Gold
            Color::Rgb(136, 192, 208), // Nord8 Frost Cyan
            Color::Rgb(143, 188, 187), // Nord7 Frost Teal
        ],
        Season::Autumn => vec![
            Color::Rgb(235, 203, 139), // Nord13 Amber Gold
            Color::Rgb(191, 97, 106),  // Nord11 Aurora Red
            Color::Rgb(208, 135, 112), // Nord12 Aurora Orange
            Color::Rgb(180, 142, 173), // Nord15 Purple
        ],
        Season::Winter => vec![
            Color::Rgb(136, 192, 208), // Frost Cyan
            Color::Rgb(236, 239, 244), // Snow White
            Color::Rgb(94, 129, 172),  // Deep Arctic Blue
            Color::Rgb(143, 188, 187), // Frost Teal
        ],
        Season::Auto => vec![
            Color::Rgb(180, 142, 173),
            Color::Rgb(136, 192, 208),
            Color::Rgb(236, 239, 244),
            Color::Rgb(143, 188, 187),
        ],
    }
}

fn render_about_modal(f: &mut Frame, area: Rect, _config: &AppConfig) {
    let shortcuts = [
        ("1, 2, 3, 4", "Spring / Summer / Autumn / Winter"),
        ("m", "Toggle Day / Night Mood"),
        ("a, ?", "Toggle About Overlay"),
        ("q, Esc", "Quit Screensaver"),
    ];

    let mut lines = vec![
        Line::from(vec![
            Span::styled("BLOOM-RUST v0.1.0", Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
        ]).alignment(Alignment::Center),
        Line::from(vec![
            Span::styled("Author: ", Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)),
            Span::styled("iapizarro", Style::default().fg(Color::Rgb(229, 233, 240))),
            Span::styled("   •   ", Style::default().fg(Color::Rgb(94, 129, 172))),
            Span::styled("Design: ", Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)),
            Span::styled("Antigravity", Style::default().fg(Color::Rgb(229, 233, 240))),
        ]).alignment(Alignment::Center),
        Line::from(""),
        Line::from(vec![
            Span::styled("──────────── ", Style::default().fg(Color::Rgb(94, 129, 172))),
            Span::styled("Keyboard Shortcuts", Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD)),
            Span::styled(" ────────────", Style::default().fg(Color::Rgb(94, 129, 172))),
        ]).alignment(Alignment::Center),
        Line::from(""),
    ];

    for (keys, desc) in shortcuts {
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(format!("{:<14}", keys), Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<32}", desc), Style::default().fg(Color::Rgb(229, 233, 240))),
        ]));
    }

    let modal_width = 56.min(area.width.saturating_sub(4));
    let modal_height = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    let rect = centered_rect(modal_width, modal_height, area);

    // 1. Clear background underneath modal rect so tree art & stars don't bleed through
    f.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(Color::Rgb(46, 52, 64)));

    let paragraph = Paragraph::new(lines)
        .block(block);

    f.render_widget(paragraph, rect);
}

