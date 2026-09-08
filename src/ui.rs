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

#[allow(dead_code)]
pub mod palette {
    use ratatui::style::Color;

    // Aurora & Sakura Spectrum
    pub const SAKURA_PURPLE: Color = Color::Rgb(180, 142, 173); // #B48EAD (Nord15)
    pub const AURORA_GREEN: Color = Color::Rgb(163, 190, 140);  // #A3BE8C (Nord14)
    pub const AURORA_YELLOW: Color = Color::Rgb(235, 203, 139); // #EBCB8B (Nord13)
    pub const AURORA_ORANGE: Color = Color::Rgb(208, 135, 112); // #D08770 (Nord12)
    pub const AURORA_RED: Color = Color::Rgb(191, 97, 106);     // #BF616A (Nord11)

    // Warm Amber & Golden Honey Gradient
    pub const GOLD_CHAMPAGNE: Color = Color::Rgb(245, 224, 179); // #F5E0B3
    pub const HONEY_GOLD: Color = Color::Rgb(223, 192, 124);     // #DFC07C
    pub const GOLDEN_OCHRE: Color = Color::Rgb(201, 166, 93);    // #C9A65D
    pub const AMBER_BRONZE: Color = Color::Rgb(163, 129, 61);    // #A3813D

    // Frost & Glacial Cyan / Teal Gradient
    pub const GLACIAL_MIST: Color = Color::Rgb(194, 229, 228);   // #C2E5E4
    pub const SOFT_FROST_TEAL: Color = Color::Rgb(168, 209, 208); // #A8D1D0
    pub const NORD_FROST_TEAL: Color = Color::Rgb(143, 188, 187); // #8FBCBB (Nord7)
    pub const NORD_FROST_CYAN: Color = Color::Rgb(136, 191, 208); // #88BFD0 (Nord8)
    pub const SEA_TEAL: Color = Color::Rgb(111, 158, 157);       // #6F9E9D
    pub const PINE_TEAL: Color = Color::Rgb(82, 126, 125);       // #527E7D
    pub const GLACIAL_BLUE: Color = Color::Rgb(129, 161, 193);   // #81A1C1 (Nord9)
    pub const ARCTIC_BLUE: Color = Color::Rgb(94, 129, 172);     // #5E81AC (Nord10)

    // Blossom Plum / Violet / Heather Gradient
    pub const SAKURA_MIST: Color = Color::Rgb(216, 191, 212);    // #D8BFD4
    pub const WISTERIA_VIOLET: Color = Color::Rgb(157, 146, 189); // #9D92BD
    pub const BLOSSOM_ROSE: Color = Color::Rgb(194, 139, 159);   // #C28B9F
    pub const DUSK_MAUVE: Color = Color::Rgb(143, 108, 137);     // #8F6C89
    pub const SHADOW_PLUM: Color = Color::Rgb(102, 73, 97);      // #664961

    // Polar Night Slate & Shadow
    pub const POLAR_SLATE_BRIGHT: Color = Color::Rgb(76, 86, 106); // #4C566A (Nord3)
    pub const POLAR_SLATE_MED: Color = Color::Rgb(67, 76, 94);     // #434C5E (Nord2)
    pub const POLAR_SLATE_DARK: Color = Color::Rgb(59, 66, 82);    // #3B4252 (Nord1)
    pub const POLAR_NIGHT_BASE: Color = Color::Rgb(46, 52, 64);    // #2E3440 (Nord0)

    // Snow Storm & Radiant Whites
    pub const SNOW_WHITE: Color = Color::Rgb(236, 239, 244);     // #ECEFF4 (Nord6)
    pub const SNOW_STORM: Color = Color::Rgb(229, 233, 240);     // #E5E9F0 (Nord5)
    pub const SNOW_MIST: Color = Color::Rgb(216, 222, 233);      // #D8DEE9 (Nord4)
}

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
        Mood::Night => palette::POLAR_NIGHT_BASE, // Nord0 Polar Night (#2E3440)
        Mood::Day => palette::SNOW_STORM,         // Nord5 Snow Storm (#E5E9F0)
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
                    ('*', palette::SNOW_WHITE)       // #ECEFF4 Peak twinkle
                } else if val > 0.4 {
                    ('+', palette::GLACIAL_MIST)     // #C2E5E4 Bright glow
                } else if val > 0.0 {
                    ('.', palette::SOFT_FROST_TEAL)  // #A8D1D0 Soft glow
                } else if val > -0.5 {
                    ('.', palette::GLACIAL_BLUE)     // #81A1C1 Dim star
                } else {
                    ('.', palette::ARCTIC_BLUE)      // #5E81AC Deep arctic star
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
                    Mood::Night => palette::POLAR_SLATE_BRIGHT, // Nord3 Slate (#4C566A)
                    Mood::Day => palette::SNOW_MIST,            // Nord4 Snow Storm Mist (#D8DEE9)
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

                    let sway_dx = if h_frac > 0.0 {
                        compute_char_sway(
                            row_sway,
                            h_frac,
                            r,
                            c,
                            art_width,
                            particles.time,
                            particles.sway,
                        )
                    } else {
                        row_sway.round().clamp(-2.0, 2.0) as i32
                    };

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
            let step_y = if target_height > 1 && art_height > 1 {
                ((art_height - 1) as f32 / (target_height - 1) as f32).max(0.001)
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
                            let sway_dx = if h_frac > 0.0 {
                                compute_char_sway(
                                    row_sway,
                                    h_frac,
                                    sy,
                                    sx,
                                    art_width,
                                    particles.time,
                                    particles.sway,
                                )
                            } else {
                                row_sway.round().clamp(-2.0, 2.0) as i32
                            };

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

    // 6. Render Organic Contoured Ground Terrain, Grass Tufts, Puddle Reflections & Floating Petals
    render_terrain_and_puddles(
        buf,
        &particles.terrain,
        &particles.floating_petals,
        blossom_colors,
        area,
        season,
        config.mood,
        nord_bg,
        current_condition,
        particles.time,
    );

    // 7. Render Settled Ground Blossom Carpet & Accumulation Mounds
    for s in &particles.settled {
        let max_y = area.height.saturating_sub(1);
        if s.x < area.width && s.y < max_y {
            let base_color = blossom_colors[s.color_idx % blossom_colors.len()];

            // Multi-stage fading gradient as petals rest and dissolve into earth
            let color = if s.alpha > 0.70 {
                base_color
            } else if s.alpha > 0.45 {
                match season {
                    Season::Spring | Season::Auto => palette::BLOSSOM_ROSE,   // #C28B9F Blossom Rose
                    Season::Summer => palette::SEA_TEAL,                     // #6F9E9D Sea Teal
                    Season::Autumn => palette::AURORA_ORANGE,                 // #D08770 Aurora Coral
                    Season::Winter => palette::NORD_FROST_CYAN,               // #88BFD0 Frost Cyan
                }
            } else if s.alpha > 0.25 {
                match season {
                    Season::Spring | Season::Auto => palette::DUSK_MAUVE,     // #8F6C89 Dusk Mauve
                    Season::Summer => palette::PINE_TEAL,                    // #527E7D Pine Teal
                    Season::Autumn => palette::AMBER_BRONZE,                  // #A3813D Bronze Amber
                    Season::Winter => palette::ARCTIC_BLUE,                   // #5E81AC Arctic Blue
                }
            } else if s.alpha > 0.10 {
                match config.mood {
                    Mood::Night => palette::ARCTIC_BLUE,                      // #5E81AC Deep Arctic Blue
                    Mood::Day => palette::GLACIAL_BLUE,                       // #81A1C1 Glacial Blue
                }
            } else {
                match config.mood {
                    Mood::Night => palette::POLAR_SLATE_BRIGHT,               // #4C566A Polar Slate
                    Mood::Day => palette::POLAR_SLATE_MED,                    // #434C5E Deep Slate
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

    // 8.5 Render Summer Night Fireflies (Hotaru)
    for ff in &particles.fireflies {
        let px = ff.x.round() as u16;
        let py = ff.y.round() as u16;
        if px < area.width && py < area.height.saturating_sub(1) {
            let brightness = (ff.pulse_phase.sin() * 0.5 + 0.5).powi(2);
            if brightness > 0.15 {
                let (glyph, col) = if brightness > 0.72 {
                    ('✦', palette::GOLD_CHAMPAGNE) // #F5E0B3 Peak Champagne glow
                } else if brightness > 0.42 {
                    ('*', palette::HONEY_GOLD)     // #DFC07C Soft Honey glow
                } else {
                    ('·', palette::SOFT_FROST_TEAL) // #A8D1D0 Soft Frost glow
                };
                set_cell(buf, px, py, glyph, col, nord_bg);
            }
        }
    }

    // 8.6 Render Perching & Flying Wildlife Birds (Japanese White-Eye / Sparrow)
    for bird in &particles.birds {
        let (draw_x, draw_y) = match bird.state {
            crate::physics::BirdFlightState::Perched => {
                let row_sway = compute_tree_row_sway(
                    bird.branch_r,
                    art_height,
                    particles.time,
                    particles.sway,
                    particles.gust_intensity,
                    particles.gust_direction,
                );
                let sway_dx = compute_char_sway(
                    row_sway.0,
                    row_sway.1,
                    bird.branch_r,
                    bird.branch_c,
                    art_width,
                    particles.time,
                    particles.sway,
                );
                (bird.target_x + sway_dx as f32, bird.target_y)
            }
            crate::physics::BirdFlightState::FlyingIn | crate::physics::BirdFlightState::FlyingOut => {
                (bird.x, bird.y)
            }
        };

        let bx = draw_x.round() as i32;
        let by = draw_y.round() as i32;

        if by >= 0 && by < area.height.saturating_sub(1) as i32 {
            let by_u = by as u16;
            match bird.state {
                crate::physics::BirdFlightState::Perched => {
                    let bird_chars = if bird.facing_right {
                        if bird.is_chirping {
                            [('>', palette::AURORA_GREEN), ('o', palette::SNOW_WHITE), ('♪', palette::HONEY_GOLD)]
                        } else {
                            [('>', palette::AURORA_GREEN), ('•', palette::SNOW_WHITE), ('>', palette::HONEY_GOLD)]
                        }
                    } else {
                        if bird.is_chirping {
                            [('♪', palette::HONEY_GOLD), ('o', palette::SNOW_WHITE), ('<', palette::AURORA_GREEN)]
                        } else {
                            [('<', palette::HONEY_GOLD), ('•', palette::SNOW_WHITE), ('<', palette::AURORA_GREEN)]
                        }
                    };
                    for (idx, &(ch, col)) in bird_chars.iter().enumerate() {
                        let px = bx + idx as i32 - 1;
                        if px >= 0 && px < area.width as i32 {
                            set_cell(buf, px as u16, by_u, ch, col, nord_bg);
                        }
                    }
                }
                crate::physics::BirdFlightState::FlyingIn | crate::physics::BirdFlightState::FlyingOut => {
                    let flap_frame = ((bird.flap_timer * 10.0) as usize) % 3;
                    let fly_chars = match flap_frame {
                        0 => ('^', 'v', '^'),
                        1 => ('~', '•', '~'),
                        _ => ('v', '•', 'v'),
                    };
                    let body_col = palette::AURORA_GREEN;
                    let eye_col = palette::SNOW_WHITE;
                    let spans = [(fly_chars.0, body_col), (fly_chars.1, eye_col), (fly_chars.2, body_col)];
                    for (idx, &(ch, col)) in spans.iter().enumerate() {
                        let px = bx + idx as i32 - 1;
                        if px >= 0 && px < area.width as i32 {
                            set_cell(buf, px as u16, by_u, ch, col, nord_bg);
                        }
                    }
                }
            }
        }
    }

    // 9. Render Dynamic Wind Gust Trails
    for streak in &particles.wind_streaks {
        let sx = streak.x.round() as u16;
        let sy = streak.y.round() as u16;
        if sx < area.width && sy < area.height.saturating_sub(1) {
            let life_frac = 1.0 - (streak.life / streak.max_life.max(0.1));
            let streak_color = if life_frac > 0.7 {
                palette::GLACIAL_MIST     // #C2E5E4
            } else if life_frac > 0.4 {
                palette::NORD_FROST_CYAN  // #88BFD0
            } else if life_frac > 0.2 {
                palette::GLACIAL_BLUE     // #81A1C1
            } else {
                palette::ARCTIC_BLUE      // #5E81AC
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
                        Mood::Night => palette::GLACIAL_BLUE, // #81A1C1
                        Mood::Day => palette::ARCTIC_BLUE,    // #5E81AC
                    },
                    WeatherFxKind::RainSplash => palette::GLACIAL_MIST, // #C2E5E4
                    WeatherFxKind::SnowFlake => match wp.phase.sin() > 0.0 {
                        true => palette::SNOW_WHITE, // #ECEFF4
                        false => palette::SNOW_MIST, // #D8DEE9
                    },
                    WeatherFxKind::SunGlimmer => match config.mood {
                        Mood::Night => palette::GLACIAL_MIST,    // #C2E5E4 Aurora Glimmer
                        Mood::Day => palette::GOLD_CHAMPAGNE,    // #F5E0B3 Sunlight Glimmer
                    },
                    _ => palette::NORD_FROST_CYAN,
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
                Mood::Night => palette::ARCTIC_BLUE,       // #5E81AC Deep Arctic Blue
                Mood::Day => palette::SEA_TEAL,           // #6F9E9D Sea Teal
            }
        } else if leaf.state == LeafState::Falling {
            // Foreground dynamic 4-tier altitude gradient shading as petals drift downward
            let progress = (py as f32 / ground_y.max(1) as f32).clamp(0.0, 1.0);
            if progress > 0.85 {
                match season {
                    Season::Spring | Season::Auto => palette::SAKURA_MIST,   // #D8BFD4 Radiant Blossom
                    Season::Summer => palette::GOLD_CHAMPAGNE,               // #F5E0B3 Champagne Sun
                    Season::Autumn => palette::HONEY_GOLD,                   // #DFC07C Honey Gold
                    Season::Winter => palette::SNOW_WHITE,                   // #ECEFF4 Pure Snow
                }
            } else if progress > 0.55 {
                match season {
                    Season::Spring | Season::Auto => palette::WISTERIA_VIOLET, // #9D92BD Wisteria Violet
                    Season::Summer => palette::AURORA_YELLOW,                // #EBCB8B Aurora Amber
                    Season::Autumn => palette::GOLDEN_OCHRE,                 // #C9A65D Golden Ochre
                    Season::Winter => palette::GLACIAL_MIST,                 // #C2E5E4 Glacial Mist
                }
            } else if progress > 0.25 {
                match season {
                    Season::Spring | Season::Auto => palette::SOFT_FROST_TEAL, // #A8D1D0 Soft Frost Teal
                    Season::Summer => palette::NORD_FROST_TEAL,               // #8FBCBB Frost Teal
                    Season::Autumn => palette::AURORA_ORANGE,                 // #D08770 Aurora Coral
                    Season::Winter => palette::NORD_FROST_CYAN,               // #88BFD0 Frost Cyan
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
    floating_petals: &[crate::physics::FloatingPetal],
    blossom_colors: &[Color],
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
                Mood::Night => palette::POLAR_SLATE_BRIGHT, // #4C566A Polar Night Slate
                Mood::Day => palette::POLAR_SLATE_MED,      // #434C5E Deep Slate
            };

            set_cell(buf, x, gy, ch, ground_color, nord_bg);
        }
    }

    // 2. Render Grass Tufts along Terrain Contour
    let grass_color = match season {
        Season::Spring | Season::Auto => match mood {
            Mood::Night => palette::WISTERIA_VIOLET, // #9D92BD Wisteria
            Mood::Day => palette::AURORA_GREEN,     // #A3BE8C Lush Green
        },
        Season::Summer => palette::AURORA_GREEN,    // #A3BE8C Lush Green
        Season::Autumn => palette::GOLDEN_OCHRE,    // #C9A65D Golden Ochre
        Season::Winter => palette::GLACIAL_MIST,    // #C2E5E4 Glacial Mist
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
                        ('≈', palette::GLACIAL_MIST)    // #C2E5E4 Active ripple crest
                    } else if wave_phase > 0.2 {
                        ('~', palette::NORD_FROST_CYAN) // #88BFD0 Frost Cyan
                    } else if wave_phase > -0.3 {
                        ('~', palette::GLACIAL_BLUE)    // #81A1C1 Glacial Blue
                    } else {
                        ('=', palette::ARCTIC_BLUE)     // #5E81AC Deep Arctic Blue
                    };

                    set_cell(buf, x, gy, puddle_ch, puddle_col, nord_bg);
                }
            }
        }

        // 4. Render Floating Blossom Petals on Puddles
        for fp in floating_petals {
            let px = fp.x.round() as u16;
            let py = fp.y;
            if px < width && py < height_limit {
                let bob = fp.bob_phase.sin() > 0.0;
                let ch = if bob { fp.ch } else { '~' };
                let frac = 1.0 - (fp.life / fp.max_life.max(0.1));
                let color = if frac > 0.60 {
                    blossom_colors[fp.color_idx % blossom_colors.len()]
                } else if frac > 0.30 {
                    palette::SOFT_FROST_TEAL // #A8D1D0
                } else {
                    palette::NORD_FROST_CYAN // #88BFD0
                };
                set_cell(buf, px, py, ch, color, nord_bg);
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
                Mood::Night => palette::POLAR_SLATE_BRIGHT, // #4C566A Polar Slate Bark
                Mood::Day => palette::POLAR_SLATE_MED,      // #434C5E Deep Slate
            },
            '*' | '+' | '=' => match mood {
                Mood::Night => palette::ARCTIC_BLUE,        // #5E81AC Deep Arctic Blue Bark
                Mood::Day => palette::PINE_TEAL,           // #527E7D Pine Teal Bark
            },
            _ => match mood {
                Mood::Night => palette::GLACIAL_BLUE,       // #81A1C1 Glacial Blue Highlight
                Mood::Day => palette::NORD_FROST_TEAL,     // #8FBCBB Frost Teal Highlight
            },
        }
    } else if is_lower_branches {
        // Transition zone between trunk and canopy
        match season {
            Season::Spring | Season::Auto => match ch {
                '%' | '#' | '@' => match mood {
                    Mood::Night => palette::SHADOW_PLUM,    // #664961 Deep Shadow Plum
                    Mood::Day => palette::ARCTIC_BLUE,      // #5E81AC Arctic Blue
                },
                '*' | '+' | '=' => match mood {
                    Mood::Night => palette::DUSK_MAUVE,     // #8F6C89 Dusk Mauve
                    Mood::Day => palette::WISTERIA_VIOLET,  // #9D92BD Wisteria Violet
                },
                _ => palette::SAKURA_MIST,                  // #D8BFD4 Sakura Mist
            },
            Season::Summer => match ch {
                '%' | '#' | '@' => palette::PINE_TEAL,      // #527E7D Pine Teal
                '*' | '+' | '=' => palette::AURORA_GREEN,   // #A3BE8C Lush Aurora Green
                _ => palette::HONEY_GOLD,                   // #DFC07C Honey Gold
            },
            Season::Autumn => match ch {
                '%' | '#' | '@' => palette::AMBER_BRONZE,   // #A3813D Deep Bronze Amber
                '*' | '+' | '=' => palette::AURORA_RED,     // #BF616A Aurora Crimson
                _ => palette::GOLDEN_OCHRE,                 // #C9A65D Golden Ochre
            },
            Season::Winter => match ch {
                '%' | '#' | '@' => palette::ARCTIC_BLUE,    // #5E81AC Deep Arctic Blue
                '*' | '+' | '=' => palette::SEA_TEAL,       // #6F9E9D Sea Teal
                _ => palette::GLACIAL_MIST,                 // #C2E5E4 Glacial Mist
            },
        }
    } else {
        // TrueColor 3D Radial Foliage Depth & Canopy Glow Model with 4-stop gradient
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

        let (c0, c1, c2, c3) = match season {
            Season::Spring | Season::Auto => match mood {
                Mood::Night => (
                    palette::SHADOW_PLUM,     // #664961 Core Deep Shadow Plum
                    palette::DUSK_MAUVE,      // #8F6C89 Mid-low Heather Mauve
                    palette::BLOSSOM_ROSE,    // #C28B9F Mid-high Blossom Rose
                    palette::SAKURA_MIST,     // #D8BFD4 Outer Radiant Sakura Mist
                ),
                Mood::Day => (
                    palette::ARCTIC_BLUE,     // #5E81AC Core Glacial Shadow
                    palette::SAKURA_PURPLE,   // #B48EAD Mid Sakura Pink
                    palette::SAKURA_MIST,     // #D8BFD4 Outer Sakura Mist
                    palette::SNOW_WHITE,      // #ECEFF4 Pure Snow Radiant Glow
                ),
            },
            Season::Summer => match mood {
                Mood::Night => (
                    palette::PINE_TEAL,       // #527E7D Core Deep Pine Teal
                    palette::SEA_TEAL,        // #6F9E9D Mid-low Sea Teal
                    palette::AURORA_GREEN,    // #A3BE8C Mid-high Lush Green
                    palette::HONEY_GOLD,      // #DFC07C Outer Amber Sunlight
                ),
                Mood::Day => (
                    palette::PINE_TEAL,       // #527E7D Core Pine Teal
                    palette::AURORA_GREEN,    // #A3BE8C Mid Lush Green
                    palette::HONEY_GOLD,      // #DFC07C Outer Honey Gold
                    palette::GOLD_CHAMPAGNE,  // #F5E0B3 Radiant Sun Glow
                ),
            },
            Season::Autumn => match mood {
                Mood::Night => (
                    palette::AMBER_BRONZE,    // #A3813D Core Bronze Amber
                    palette::AURORA_RED,      // #BF616A Mid-low Crimson
                    palette::AURORA_ORANGE,   // #D08770 Mid-high Coral
                    palette::HONEY_GOLD,      // #DFC07C Outer Golden Glow
                ),
                Mood::Day => (
                    palette::AURORA_RED,      // #BF616A Core Crimson
                    palette::AURORA_ORANGE,   // #D08770 Mid Coral
                    palette::GOLDEN_OCHRE,    // #C9A65D Mid Golden Ochre
                    palette::GOLD_CHAMPAGNE,  // #F5E0B3 Radiant Sunlight Glow
                ),
            },
            Season::Winter => match mood {
                Mood::Night => (
                    palette::ARCTIC_BLUE,     // #5E81AC Core Deep Arctic
                    palette::SEA_TEAL,        // #6F9E9D Mid Sea Teal
                    palette::NORD_FROST_CYAN, // #88BFD0 Mid Frost Cyan
                    palette::GLACIAL_MIST,    // #C2E5E4 Outer Glacial Glow
                ),
                Mood::Day => (
                    palette::GLACIAL_BLUE,    // #81A1C1 Core Glacial
                    palette::NORD_FROST_CYAN, // #88BFD0 Mid Frost Cyan
                    palette::GLACIAL_MIST,    // #C2E5E4 Mid Glacial Mist
                    palette::SNOW_WHITE,      // #ECEFF4 Pure Snow Radiant Glow
                ),
            },
        };

        if depth < 0.33 {
            lerp_color(c0, c1, depth / 0.33)
        } else if depth < 0.66 {
            lerp_color(c1, c2, (depth - 0.33) / 0.33)
        } else {
            lerp_color(c2, c3, (depth - 0.66) / 0.34)
        }
    }
}

pub fn get_effective_season(config: &AppConfig, weather: &WeatherFetcher) -> Season {
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
    use palette::*;
    match season {
        Season::Spring | Season::Auto => &[
            SAKURA_MIST,       // #D8BFD4 Radiant Blossom Mist
            SAKURA_PURPLE,     // #B48EAD Nordic Sakura Pink
            BLOSSOM_ROSE,      // #C28B9F Blossom Rose
            WISTERIA_VIOLET,   // #9D92BD Wisteria Violet
            SOFT_FROST_TEAL,   // #A8D1D0 Soft Frost Teal
            NORD_FROST_CYAN,   // #88BFD0 Frost Cyan
            GLACIAL_MIST,      // #C2E5E4 Glacial Mist
            SNOW_WHITE,        // #ECEFF4 Pure Snow White
        ],
        Season::Summer => &[
            AURORA_GREEN,      // #A3BE8C Lush Aurora Green
            GOLD_CHAMPAGNE,    // #F5E0B3 Champagne Sun
            HONEY_GOLD,        // #DFC07C Honey Gold
            AURORA_YELLOW,     // #EBCB8B Warm Amber
            SOFT_FROST_TEAL,   // #A8D1D0 Soft Frost
            NORD_FROST_TEAL,   // #8FBCBB Frost Teal
            SEA_TEAL,          // #6F9E9D Sea Teal
            NORD_FROST_CYAN,   // #88BFD0 Frost Cyan
        ],
        Season::Autumn => &[
            GOLD_CHAMPAGNE,    // #F5E0B3 Golden Sunlight
            HONEY_GOLD,        // #DFC07C Honey Gold
            AURORA_YELLOW,     // #EBCB8B Amber Gold
            GOLDEN_OCHRE,      // #C9A65D Golden Ochre
            AURORA_ORANGE,     // #D08770 Aurora Coral
            AURORA_RED,        // #BF616A Aurora Crimson
            AMBER_BRONZE,      // #A3813D Deep Bronze
            DUSK_MAUVE,        // #8F6C89 Wine Mauve
        ],
        Season::Winter => &[
            SNOW_WHITE,        // #ECEFF4 Pure Snow
            SNOW_STORM,        // #E5E9F0 Snow Storm
            GLACIAL_MIST,      // #C2E5E4 Glacial Mist
            SOFT_FROST_TEAL,   // #A8D1D0 Soft Cyan
            NORD_FROST_CYAN,   // #88BFD0 Frost Cyan
            GLACIAL_BLUE,      // #81A1C1 Glacial Blue
            SEA_TEAL,          // #6F9E9D Deep Frost
            ARCTIC_BLUE,       // #5E81AC Deep Arctic
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
        ("q", "Quit Screensaver"),
    ];

    let is_compact = area.height < 18;

    let mut lines = vec![
        Line::from(vec![
            Span::styled(format!("BLOOM-RUST v{}", env!("CARGO_PKG_VERSION")), Style::default().fg(Color::Rgb(94, 129, 172)).add_modifier(Modifier::BOLD)),
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
        }
    } else {
        lines.push(
            Line::from(vec![
                Span::styled("  Telemetry data is currently synchronizing with Open-Meteo...", Style::default().fg(Color::Rgb(143, 188, 187))),
            ])
            .alignment(Alignment::Center),
        );
    }

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
    if r >= trunk_threshold || trunk_threshold == 0 {
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
    let inv_center_c = 1.0 / center_c.max(1.0);
    let dist_c = ((c as f32 - center_c).abs() * inv_center_c).clamp(0.0, 1.0);
    let tip_flutter = if dist_c > 0.25 {
        let flutter_phase = time * 3.0 + (r.wrapping_mul(5).wrapping_add(c)) as f32 * 0.25;
        flutter_phase.sin() * (0.45 * sway * dist_c * h_frac)
    } else {
        0.0
    };
    (row_base_sway + tip_flutter).round().clamp(-2.0, 2.0) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_color_cache() {
        let mut cache = TreeColorCache::default();
        let grid = vec![vec!['#', ' ', '*']];
        let colors1 = cache.get_or_compute(&grid, Season::Spring, Mood::Night).to_vec();
        assert_eq!(colors1.len(), 1);
        assert_eq!(colors1[0].len(), 3);
        assert_eq!(colors1[0][1], Color::Reset);

        // Fetching again with same parameters uses cache
        let colors2 = cache.get_or_compute(&grid, Season::Spring, Mood::Night);
        assert_eq!(colors1[0], colors2[0]);
    }

    #[test]
    fn test_sway_computations_edge_cases() {
        // Zero art dimensions should not panic
        let (row_sway, h_frac) = compute_tree_row_sway(0, 0, 0.0, 1.0, 0.0, 1.0);
        assert_eq!(row_sway, 0.0);
        assert_eq!(h_frac, 0.0);

        let char_sway = compute_char_sway(0.0, 0.0, 0, 0, 0, 0.0, 1.0);
        assert_eq!(char_sway, 0);

        // Normal sway values
        let (row_sway_normal, h_frac_normal) = compute_tree_row_sway(5, 50, 1.0, 1.0, 0.5, 1.0);
        assert!(h_frac_normal > 0.0);
        let char_sway_normal = compute_char_sway(row_sway_normal, h_frac_normal, 5, 20, 80, 1.0, 1.0);
        assert!((-2..=2).contains(&char_sway_normal));
    }
}
