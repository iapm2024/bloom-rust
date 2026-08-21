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
use crate::weather::{weathercode_full_name, weathercode_symbol, WeatherCondition, WeatherFetcher, WeatherFxEngine, WeatherFxKind};
use chrono::{Datelike, Local};
use unicode_width::UnicodeWidthStr;

use std::cell::RefCell;

#[derive(Default)]
pub struct TreeColorCache {
    cached_season: Option<Season>,
    cached_mood: Option<Mood>,
    cached_grid_hash: usize,
    pub color_grid: Vec<Vec<Color>>,
}

impl TreeColorCache {
    pub fn get_or_compute(
        &mut self,
        tree_grid: &[Vec<char>],
        season: Season,
        mood: Mood,
    ) -> &[Vec<Color>] {
        let grid_len = tree_grid.len();
        let needs_recompute = self.cached_season != Some(season)
            || self.cached_mood != Some(mood)
            || self.cached_grid_hash != grid_len
            || self.color_grid.is_empty();

        if needs_recompute {
            let art_height = tree_grid.len();
            let art_width = tree_grid.iter().map(|r| r.len()).max().unwrap_or(80);
            let mut new_grid = Vec::with_capacity(art_height);

            for (r, row) in tree_grid.iter().enumerate() {
                let mut row_colors = Vec::with_capacity(row.len());
                for (c, &ch) in row.iter().enumerate() {
                    if ch == ' ' {
                        row_colors.push(Color::Reset);
                    } else {
                        let col = compute_tree_char_color(ch, r, c, art_height, art_width, season, mood);
                        row_colors.push(col);
                    }
                }
                new_grid.push(row_colors);
            }

            self.color_grid = new_grid;
            self.cached_season = Some(season);
            self.cached_mood = Some(mood);
            self.cached_grid_hash = grid_len;
        }

        &self.color_grid
    }
}

#[derive(Default)]
struct StatusLineCache {
    last_sec: i64,
    last_weather_str: String,
    last_season: Option<Season>,
    cached_status_line: String,
}

thread_local! {
    static TREE_COLOR_CACHE: RefCell<TreeColorCache> = RefCell::new(TreeColorCache::default());
    static STATUS_CACHE: RefCell<StatusLineCache> = RefCell::new(StatusLineCache::default());
}

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
    show_weather: bool,
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

    // 5. Render Static Tree Matrix with Cached Colors & High-Performance Row-Level Sway
    let art_height = particles.art_height;
    let art_width = particles.art_width;
    let target_height = area.height.saturating_sub(1) as usize;

    TREE_COLOR_CACHE.with(|cache_cell| {
        let mut cache = cache_cell.borrow_mut();
        let tree_colors = cache.get_or_compute(tree_grid, season, config.mood);

        if art_height <= target_height {
            // 1:1 scale rendering with harmonic canopy sway & branch flutter
            let offset_x = (area.width as usize).saturating_sub(art_width) / 2;
            let offset_y = target_height - art_height;

            for (r, row) in tree_grid.iter().enumerate() {
                let py = (offset_y + r) as u16;
                if py >= area.height.saturating_sub(1) {
                    break;
                }

                let (row_sway, h_frac) = compute_tree_row_sway(
                    r,
                    art_height,
                    particles.time,
                    particles.sway,
                    particles.gust_intensity,
                    particles.gust_direction,
                );

                for (c, &ch) in row.iter().enumerate() {
                    if ch == ' ' {
                        continue;
                    }

                    let sway_dx = compute_char_sway(
                        row_sway,
                        h_frac,
                        r,
                        c,
                        art_width,
                        particles.time,
                        particles.sway,
                    );

                    let px_i32 = offset_x as i32 + c as i32 + sway_dx;
                    if px_i32 >= 0 && px_i32 < area.width as i32 {
                        let px = px_i32 as u16;
                        let color = tree_colors.get(r).and_then(|rc| rc.get(c)).copied().unwrap_or(Color::Reset);
                        set_cell(buf, px, py, ch, color, nord_bg);
                    }
                }
            }
        } else {
            // Proportional Scaling to fit inside small terminal height with organic sway
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

                    let (row_sway, h_frac) = compute_tree_row_sway(
                        sy,
                        art_height,
                        particles.time,
                        particles.sway,
                        particles.gust_intensity,
                        particles.gust_direction,
                    );

                    for bx in 0..scaled_cols {
                        let sx = ((bx as f32 * step_x).round() as usize).min(row_len.saturating_sub(1));
                        let ch = row[sx];
                        if ch != ' ' {
                            let sway_dx = compute_char_sway(
                                row_sway,
                                h_frac,
                                sy,
                                sx,
                                art_width,
                                particles.time,
                                particles.sway,
                            );

                            let px_i32 = offset_x as i32 + bx as i32 + sway_dx;
                            let py = by as u16;

                            if px_i32 >= 0 && px_i32 < area.width as i32 && py < area.height.saturating_sub(1) {
                                let px = px_i32 as u16;
                                let color = tree_colors.get(sy).and_then(|rc| rc.get(sx)).copied().unwrap_or(Color::Reset);
                                set_cell(buf, px, py, ch, color, nord_bg);
                            }
                        }
                    }
                }
            }
        }
    });

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
    let footer_rect = Rect::new(0, area.height.saturating_sub(1), area.width, 1);
    let footer_p = if let Some(feedback) = particles.get_active_feedback() {
        Paragraph::new(format!("─── [ {} ] ───", feedback))
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(136, 192, 208)).bg(nord_bg).add_modifier(Modifier::BOLD))
    } else {
        let status_line = STATUS_CACHE.with(|cache_cell| {
            let mut cache = cache_cell.borrow_mut();
            let now = Local::now();
            let cur_sec = now.timestamp();

            if cache.last_sec != cur_sec || cache.last_season != Some(season) {
                let weather_str = weather.get_weather_info();
                if cache.last_weather_str != weather_str || cache.last_sec != cur_sec || cache.last_season != Some(season) {
                    let date_str = now.format("%A %d/%m/%Y");
                    let season_str = match season {
                        Season::Spring => "Spring",
                        Season::Summer => "Summer",
                        Season::Autumn => "Autumn",
                        Season::Winter => "Winter",
                        Season::Auto => "Auto",
                    };
                    cache.cached_status_line = format!("{}  │  {}  │  {}", date_str, weather_str, season_str);
                    cache.last_sec = cur_sec;
                    cache.last_season = Some(season);
                    cache.last_weather_str = weather_str;
                }
            }

            cache.cached_status_line.clone()
        });

        Paragraph::new(status_line)
            .alignment(Alignment::Center)
            .style(Style::default().fg(Color::Rgb(129, 161, 193)).bg(nord_bg).add_modifier(Modifier::BOLD))
    };
    f.render_widget(footer_p, footer_rect);

    // 12. Render Modal Overlays if toggled
    if show_weather {
        render_weather_modal(f, area, weather, config);
    } else if show_about {
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

fn compute_tree_char_color(
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
        ("0", "Reset Season to Auto (Astronomical)"),
        ("m", "Toggle Day / Night Mood"),
        ("f", "Weather Forecast Pop-up"),
        ("r", "Refresh Live Weather Data"),
        ("g", "Trigger Wind Gust Surge"),
        ("a, ?", "Toggle About Overlay"),
        ("q, Esc", "Quit Screensaver"),
    ];

    let is_compact = area.height < 18;

    let mut lines = vec![
        Line::from(vec![
            Span::styled("BLOOM-RUST v0.4.0", Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
        ]).alignment(Alignment::Center),
        Line::from(vec![
            Span::styled("Author: ", Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)),
            Span::styled("iapizarro", Style::default().fg(Color::Rgb(229, 233, 240))),
            Span::styled("   *   ", Style::default().fg(Color::Rgb(94, 129, 172))),
            Span::styled("Design: ", Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)),
            Span::styled("Antigravity", Style::default().fg(Color::Rgb(229, 233, 240))),
        ]).alignment(Alignment::Center),
    ];

    if !is_compact {
        lines.push(Line::from(""));
    }

    lines.push(
        Line::from(vec![
            Span::styled("──────────── ", Style::default().fg(Color::Rgb(94, 129, 172))),
            Span::styled("Keyboard Shortcuts", Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD)),
            Span::styled(" ────────────", Style::default().fg(Color::Rgb(94, 129, 172))),
        ]).alignment(Alignment::Center),
    );

    if !is_compact {
        lines.push(Line::from(""));
    }

    for (keys, desc) in shortcuts {
        lines.push(Line::from(vec![
            Span::styled("    ", Style::default()),
            Span::styled(format!("{:<14}", keys), Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{:<32}", desc), Style::default().fg(Color::Rgb(229, 233, 240))),
        ]));
    }

    let modal_width = 58.min(area.width.saturating_sub(4));
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

fn center_glyph_text(s: &str, total_width: usize) -> String {
    let w = s.width();
    if w >= total_width {
        return s.to_string();
    }
    let pad = total_width - w;
    let left = pad / 2;
    let right = pad - left;
    format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
}

fn render_weather_modal(f: &mut Frame, area: Rect, weather: &WeatherFetcher, _config: &AppConfig) {
    let snapshot = weather.get_detailed_snapshot();
    let mut lines = Vec::new();
    let is_compact = area.height < 24;

    // 1. Header Title
    lines.push(
        Line::from(vec![
            Span::styled(
                "WEATHER FORECAST",
                Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD),
            ),
        ])
        .alignment(Alignment::Center),
    );

    // 2. Location & Update Timestamp
    let updated_str = snapshot.last_updated.as_deref().unwrap_or("Updating...");
    lines.push(
        Line::from(vec![
            Span::styled("Location: ", Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)),
            Span::styled(&snapshot.location_name, Style::default().fg(Color::Rgb(229, 233, 240))),
            Span::styled("   *   ", Style::default().fg(Color::Rgb(94, 129, 172))),
            Span::styled("Updated: ", Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)),
            Span::styled(updated_str, Style::default().fg(Color::Rgb(229, 233, 240))),
        ])
        .alignment(Alignment::Center),
    );

    // 3. Current Temperature & Condition
    if let Some(ref cur) = snapshot.primary {
        let sym = weathercode_symbol(cur.weathercode);
        let full_desc = weathercode_full_name(cur.weathercode);
        lines.push(
            Line::from(vec![
                Span::styled("Current: ", Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD)),
                Span::styled(format!("{:.1}°C", cur.temp), Style::default().fg(Color::Rgb(229, 233, 240)).add_modifier(Modifier::BOLD)),
                Span::styled("   │   ", Style::default().fg(Color::Rgb(94, 129, 172))),
                Span::styled(format!("{}  {}", sym, full_desc), Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD)),
            ])
            .alignment(Alignment::Center),
        );

        if !is_compact {
            lines.push(Line::from(""));
        }

        // 4. Hourly Forecast Timeline Chart (GNOME Weather style)
        if !cur.hourly.is_empty() {
            lines.push(
                Line::from(vec![
                    Span::styled("──────────── ", Style::default().fg(Color::Rgb(94, 129, 172))),
                    Span::styled("Hourly Forecast", Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD)),
                    Span::styled(" ────────────", Style::default().fg(Color::Rgb(94, 129, 172))),
                ])
                .alignment(Alignment::Center),
            );

            if !is_compact {
                lines.push(Line::from(""));
            }

            let num_points = cur.hourly.len().min(10);
            let hourly_slice = &cur.hourly[..num_points];

            let min_temp = hourly_slice.iter().map(|h| h.temp).fold(f64::INFINITY, f64::min);
            let max_temp = hourly_slice.iter().map(|h| h.temp).fold(f64::NEG_INFINITY, f64::max);
            let range = if (max_temp - min_temp).abs() < 0.2 { 1.0 } else { max_temp - min_temp };

            // Line 1: Time Labels
            let mut time_spans = vec![Span::styled("  ", Style::default())];
            for h in hourly_slice {
                time_spans.push(Span::styled(
                    format!("{:^7}", h.time_label),
                    Style::default().fg(Color::Rgb(129, 161, 193)).add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(time_spans));

            // Line 2: Weather Glyphs / Symbols (e.g. ☀, ☁, ☂, ❄, ❅, ☇, ≡, ⁘)
            let mut cond_spans = vec![Span::styled("  ", Style::default())];
            for h in hourly_slice {
                let sym = weathercode_symbol(h.weathercode);
                cond_spans.push(Span::styled(
                    center_glyph_text(sym, 7),
                    Style::default().fg(Color::Rgb(143, 188, 187)).add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(cond_spans));

            // Lines 3-5: Temperature Trend Curve (3 vertical rows)
            let heights: Vec<usize> = hourly_slice
                .iter()
                .map(|h| {
                    let frac = ((h.temp - min_temp) / range).clamp(0.0, 1.0);
                    (frac * 2.0).round() as usize
                })
                .collect();

            for row in (0..3).rev() {
                let mut curve_spans = vec![Span::styled("  ", Style::default())];
                for &h in &heights {
                    let (glyph, col) = if h == row {
                        (" ╭───╮ ", Color::Rgb(143, 188, 187)) // Nord7 Frost Teal curve line
                    } else if h > row {
                        (" │   │ ", Color::Rgb(94, 129, 172))  // Nord10 Deep Arctic Blue shaded fill
                    } else {
                        ("       ", Color::Reset)
                    };
                    curve_spans.push(Span::styled(glyph, Style::default().fg(col)));
                }
                lines.push(Line::from(curve_spans));
            }

            // Line 6: Numerical Temperature Values
            let mut temp_spans = vec![Span::styled("  ", Style::default())];
            for h in hourly_slice {
                temp_spans.push(Span::styled(
                    format!("{:^7}", format!("{:.0}°", h.temp)),
                    Style::default().fg(Color::Rgb(229, 233, 240)).add_modifier(Modifier::BOLD),
                ));
            }
            lines.push(Line::from(temp_spans));

            if !is_compact {
                lines.push(Line::from(""));
            }
        }
    } else {
        lines.push(
            Line::from(vec![
                Span::styled("  Telemetry data is currently synchronizing with Open-Meteo...", Style::default().fg(Color::Rgb(143, 188, 187))),
            ])
            .alignment(Alignment::Center),
        );
        if !is_compact {
            lines.push(Line::from(""));
        }
    }

    // 6. Navigation Footer
    lines.push(
        Line::from(vec![
            Span::styled("f", Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
            Span::styled("  Close Forecast", Style::default().fg(Color::Rgb(229, 233, 240))),
            Span::styled("   *   ", Style::default().fg(Color::Rgb(94, 129, 172))),
            Span::styled("r", Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
            Span::styled("  Refresh Telemetry", Style::default().fg(Color::Rgb(229, 233, 240))),
            Span::styled("   *   ", Style::default().fg(Color::Rgb(94, 129, 172))),
            Span::styled("q, Esc", Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
            Span::styled("  Exit Screensaver", Style::default().fg(Color::Rgb(229, 233, 240))),
        ])
        .alignment(Alignment::Center),
    );

    let modal_width = 76.min(area.width.saturating_sub(4));
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

#[inline]
fn compute_tree_row_sway(
    r: usize,
    art_height: usize,
    time: f32,
    sway: f32,
    gust_intensity: f32,
    gust_direction: f32,
) -> (f32, f32) {
    let trunk_threshold = (art_height * 65) / 100;
    if r >= trunk_threshold {
        return (0.0, 0.0);
    }
    let h_frac = (1.0 - (r as f32 / trunk_threshold as f32)).clamp(0.0, 1.0);
    let ambient_wave = (time * 1.6 + (r as f32 * 0.12)).sin() * (0.6 * sway) * (h_frac * h_frac);
    let gust_bend = gust_intensity * gust_direction * 1.5 * h_frac.powf(1.4);
    (ambient_wave + gust_bend, h_frac)
}

#[inline]
fn compute_char_sway(
    row_base_sway: f32,
    h_frac: f32,
    r: usize,
    c: usize,
    art_width: usize,
    time: f32,
    sway: f32,
) -> i32 {
    if h_frac <= 0.0 {
        return 0;
    }
    let center_c = (art_width as f32) * 0.5;
    let dist_c = ((c as f32 - center_c).abs() / center_c.max(1.0)).clamp(0.0, 1.0);
    let tip_flutter = if dist_c > 0.25 {
        (time * 3.0 + (r * 5 + c) as f32 * 0.25).sin() * (0.45 * sway * dist_c * h_frac)
    } else {
        0.0
    };
    (row_base_sway + tip_flutter).round().clamp(-2.0, 2.0) as i32
}
