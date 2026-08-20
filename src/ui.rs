use ratatui::{
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::config::{AppConfig, Hemisphere, Mood, Season};
use crate::physics::{Leaf, LeafState, ParallaxLayer, ParticleEngine};
use crate::stars::StarrySky;
use crate::weather::{WeatherCondition, WeatherFetcher, WeatherFxEngine, WeatherFxKind};
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
    let current_condition = weather.get_primary_condition();

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

    // 3. Render Background Weather Particle FX (Fog wisps & Cloud wisps)
    for wp in &weather_fx.particles {
        if matches!(wp.kind, WeatherFxKind::FogWisp | WeatherFxKind::CloudWisp) {
            let px = wp.x.round() as u16;
            let py = wp.y.round() as u16;
            if px < area.width && py < area.height.saturating_sub(1) {
                let color = match config.mood {
                    Mood::Night => Color::Rgb(76, 86, 106),  // Nord3 Polar Night Slate (#4C566A)
                    Mood::Day => Color::Rgb(216, 222, 233),  // Nord4 Snow Storm Mist
                };
                set_cell(buf, px, py, wp.ch, color, nord_bg);
            }
        }
    }

    // 4. Parallax Depth: Render BACKGROUND Petals (Behind the Tree Matrix)
    for leaf in &particles.leaves {
        if leaf.layer == ParallaxLayer::Background {
            render_leaf(buf, leaf, area, blossom_colors, season, config.mood, nord_bg, particles);
        }
    }

    // 5. Render Static Tree Matrix (Adaptive scaling: 1:1 if it fits, or proportional downsampling)
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

    // 6. Render Organic Contoured Ground Terrain, Grass Tufts & Puddle Reflections
    render_terrain_and_puddles(buf, &particles.terrain, area, season, config.mood, nord_bg, current_condition, particles.time);

    // 7. Render Settled Ground Blossom Carpet & Accumulation Mounds
    for s in &particles.settled {
        let max_y = area.height.saturating_sub(1);
        if s.x < area.width && s.y < max_y {
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

    // 8. Parallax Depth: Render FOREGROUND Petals (In Front of Tree Matrix with Full Tumbling)
    for leaf in &particles.leaves {
        if leaf.layer == ParallaxLayer::Foreground {
            render_leaf(buf, leaf, area, blossom_colors, season, config.mood, nord_bg, particles);
        }
    }

    // 9. Render Dynamic Wind Gust Trails
    for streak in &particles.wind_streaks {
        let sx = streak.x.round() as u16;
        let sy = streak.y.round() as u16;
        if sx < area.width && sy < area.height.saturating_sub(1) {
            let life_frac = 1.0 - (streak.life / streak.max_life.max(0.1));
            let streak_color = if life_frac > 0.6 {
                Color::Rgb(136, 192, 208) // Nord8 Frost Cyan
            } else if life_frac > 0.3 {
                Color::Rgb(129, 161, 193) // Nord9 Glacial Blue
            } else {
                Color::Rgb(94, 129, 172)  // Nord10 Deep Arctic Blue
            };
            set_cell(buf, sx, sy, streak.ch, streak_color, nord_bg);
        }
    }

    // 10. Render Foreground Weather Particle FX (Raindrops, Splashes, Snowflakes, Sun Glimmers)
    for wp in &weather_fx.particles {
        if !matches!(wp.kind, WeatherFxKind::FogWisp | WeatherFxKind::CloudWisp) {
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
                    WeatherFxKind::SunGlimmer => match config.mood {
                        Mood::Night => Color::Rgb(136, 192, 208), // Nord8 Aurora Cyan Glimmer
                        Mood::Day => Color::Rgb(235, 203, 139),   // Nord13 Amber Sunlight Glimmer
                    },
                    _ => Color::Rgb(136, 192, 208),
                };

                set_cell(buf, px, py, wp.ch, color, nord_bg);
            }
        }
    }

    // 11. Render Clean Status Bar Footer (bottom line, strictly non-emoji)
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

    // 12. Render About Modal Overlay if toggled
    if show_about {
        render_about_modal(f, area, config);
    }
}

fn render_leaf(
    buf: &mut ratatui::buffer::Buffer,
    leaf: &Leaf,
    area: Rect,
    blossom_colors: &[Color],
    season: Season,
    mood: Mood,
    nord_bg: Color,
    particles: &ParticleEngine,
) {
    let mut draw_x = leaf.x;
    if leaf.state == LeafState::Attached {
        let gust_sway = particles.sway_amplitude + (particles.gust_intensity * 1.5);
        draw_x += (leaf.phase + particles.time * 2.2).sin() * (gust_sway * 0.4);
    }

    let px = draw_x.round() as u16;
    let py = leaf.y.round() as u16;
    let ground_y = particles.terrain.get_ground_y(px.min(area.width.saturating_sub(1)));

    if px < area.width && py <= ground_y {
        let base_color = blossom_colors[leaf.color_idx % blossom_colors.len()];

        let color = if leaf.layer == ParallaxLayer::Background {
            // Background leaves have subtle atmospheric distance shading
            match mood {
                Mood::Night => Color::Rgb(94, 129, 172),  // Nord10 Deep Arctic Blue
                Mood::Day => Color::Rgb(143, 188, 187),   // Nord7 Frost Teal
            }
        } else if leaf.state == LeafState::Falling {
            // Foreground dynamic altitude gradient shading as petals drift downward
            let progress = (py as f32 / ground_y.max(1) as f32).clamp(0.0, 1.0);
            if progress > 0.85 {
                match season {
                    Season::Spring | Season::Auto => Color::Rgb(216, 222, 233), // Nord4 Snow Storm
                    Season::Summer => Color::Rgb(229, 233, 240), // Nord5 Snow
                    Season::Autumn => Color::Rgb(235, 203, 139), // Nord13 Amber
                    Season::Winter => Color::Rgb(236, 239, 244), // Nord6 Snow
                }
            } else if progress > 0.45 {
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

        set_cell(buf, px, py, leaf.current_glyph(), color, nord_bg);
    }
}

fn render_terrain_and_puddles(
    buf: &mut ratatui::buffer::Buffer,
    terrain: &crate::physics::TerrainProfile,
    area: Rect,
    season: Season,
    mood: Mood,
    nord_bg: Color,
    condition: WeatherCondition,
    time: f32,
) {
    let width = area.width;
    let height_limit = area.height.saturating_sub(1);

    // 1. Render Undulating Baseline Contour
    for x in 0..width {
        let gy = terrain.get_ground_y(x);
        if gy < height_limit {
            let left_y = if x > 0 { terrain.get_ground_y(x - 1) } else { gy };
            let right_y = if x + 1 < width { terrain.get_ground_y(x + 1) } else { gy };

            let ch = if gy < left_y {
                '/'
            } else if gy < right_y {
                '\\'
            } else if (x + gy) % 4 == 0 {
                '~'
            } else if (x + gy) % 3 == 0 {
                '-'
            } else {
                '_'
            };

            let ground_color = match mood {
                Mood::Night => Color::Rgb(76, 86, 106),  // Nord3 Polar Night Slate (#4C566A)
                Mood::Day => Color::Rgb(67, 76, 94),    // Nord2 Deep Slate (#434C5E)
            };

            set_cell(buf, x, gy, ch, ground_color, nord_bg);
        }
    }

    // 2. Render Grass Tufts along Terrain Contour
    let grass_color = match season {
        Season::Spring | Season::Auto => match mood {
            Mood::Night => Color::Rgb(143, 188, 187), // Nord7 Frost Teal
            Mood::Day => Color::Rgb(163, 190, 140),   // Nord14 Aurora Green
        },
        Season::Summer => Color::Rgb(163, 190, 140),  // Nord14 Lush Green
        Season::Autumn => Color::Rgb(208, 135, 112),  // Nord12 Coral Amber
        Season::Winter => Color::Rgb(136, 192, 208),  // Nord8 Frost Cyan
    };

    for tuft in &terrain.grass_tufts {
        let py = tuft.y;
        if py < height_limit {
            for (idx, ch) in tuft.glyph.chars().enumerate() {
                let px = tuft.x + idx as u16;
                if px < width {
                    set_cell(buf, px, py, ch, grass_color, nord_bg);
                }
            }
        }
    }

    // 3. Render Puddle Reflections & Ripple Waves During Rain
    if condition == WeatherCondition::Rain {
        for &(start_x, end_x) in &terrain.puddles {
            for x in start_x..end_x.min(width) {
                let gy = terrain.get_ground_y(x);
                if gy < height_limit {
                    let wave_phase = (x as f32 * 0.45 + time * 3.5).sin();
                    let (puddle_ch, puddle_col) = if wave_phase > 0.6 {
                        ('≈', Color::Rgb(136, 192, 208)) // Nord8 Frost Cyan (Active ripple)
                    } else if wave_phase > 0.0 {
                        ('~', Color::Rgb(129, 161, 193)) // Nord9 Glacial Blue
                    } else {
                        ('=', Color::Rgb(94, 129, 172))  // Nord10 Deep Arctic Blue
                    };

                    set_cell(buf, x, gy, puddle_ch, puddle_col, nord_bg);
                }
            }
        }
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
        // TrueColor 3D Radial Foliage Depth & Canopy Glow Model
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

        let depth = (radial_dist * 0.65 + char_weight * 0.35).clamp(0.0, 1.0);

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
        match (month, day) {
            (12, 21..=31) | (1, _) | (2, _) | (3, 1..=20) => Season::Summer,
            (3, 21..=31) | (4, _) | (5, _) | (6, 1..=20) => Season::Autumn,
            (6, 21..=31) | (7, _) | (8, _) | (9, 1..=20) => Season::Winter,
            _ => Season::Spring,
        }
    } else {
        match (month, day) {
            (12, 21..=31) | (1, _) | (2, _) | (3, 1..=20) => Season::Winter,
            (3, 21..=31) | (4, _) | (5, _) | (6, 1..=20) => Season::Spring,
            (6, 21..=31) | (7, _) | (8, _) | (9, 1..=20) => Season::Summer,
            _ => Season::Autumn,
        }
    }
}

fn get_seasonal_colors(season: Season) -> &'static [Color] {
    match season {
        Season::Spring => &[
            Color::Rgb(180, 142, 173), // Nord15 Nordic Purple
            Color::Rgb(136, 192, 208), // Nord8 Frost Cyan
            Color::Rgb(236, 239, 244), // Nord6 Snow White
            Color::Rgb(143, 188, 187), // Nord7 Frost Teal
        ],
        Season::Summer => &[
            Color::Rgb(163, 190, 140), // Nord14 Aurora Green
            Color::Rgb(235, 203, 139), // Nord13 Warm Gold
            Color::Rgb(136, 192, 208), // Nord8 Frost Cyan
            Color::Rgb(143, 188, 187), // Nord7 Frost Teal
        ],
        Season::Autumn => &[
            Color::Rgb(235, 203, 139), // Nord13 Amber Gold
            Color::Rgb(191, 97, 106),  // Nord11 Aurora Red
            Color::Rgb(208, 135, 112), // Nord12 Aurora Orange
            Color::Rgb(180, 142, 173), // Nord15 Purple
        ],
        Season::Winter => &[
            Color::Rgb(136, 192, 208), // Frost Cyan
            Color::Rgb(236, 239, 244), // Snow White
            Color::Rgb(94, 129, 172),  // Deep Arctic Blue
            Color::Rgb(143, 188, 187), // Frost Teal
        ],
        Season::Auto => &[
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
            Span::styled("BLOOM-RUST v0.3.0", Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
        ]).alignment(Alignment::Center),
        Line::from(vec![
            Span::styled("Author: ", Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)),
            Span::styled("iapizarro", Style::default().fg(Color::Rgb(229, 233, 240))),
            Span::styled("   *   ", Style::default().fg(Color::Rgb(94, 129, 172))),
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

    f.render_widget(Clear, rect);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD))
        .style(Style::default().bg(Color::Rgb(46, 52, 64)));

    let paragraph = Paragraph::new(lines).block(block);

    f.render_widget(paragraph, rect);
}
