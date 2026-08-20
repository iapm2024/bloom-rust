use rand::Rng;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParallaxLayer {
    Background,
    Foreground,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeafState {
    Attached,
    Falling,
    Settled,
    Fading,
}

#[derive(Debug, Clone)]
pub struct Leaf {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub phase: f32,
    pub speed: f32,
    pub state: LeafState,
    pub base_ch: char,
    pub color_idx: usize,
    pub layer: ParallaxLayer,
    pub angle: f32,
    pub angular_velocity: f32,
}

impl Leaf {
    pub fn current_glyph(&self) -> char {
        match self.state {
            LeafState::Attached => self.base_ch,
            LeafState::Falling => {
                let norm = self.angle.rem_euclid(std::f32::consts::PI * 2.0);
                let octant = ((norm / (std::f32::consts::PI / 4.0)).round() as usize) % 8;
                match self.layer {
                    ParallaxLayer::Foreground => match octant {
                        0 => '*',
                        1 => '/',
                        2 => '|',
                        3 => '\\',
                        4 => '+',
                        5 => '~',
                        6 => '·',
                        _ => '.',
                    },
                    ParallaxLayer::Background => match octant {
                        0 | 4 => '.',
                        1 | 5 => '·',
                        2 | 6 => '~',
                        _ => '+',
                    },
                }
            }
            LeafState::Settled | LeafState::Fading => self.base_ch,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SettledBlossom {
    pub x: u16,
    pub y: u16,
    pub ch: char,
    pub color_idx: usize,
    pub alpha: f32,
    pub decay_rate: f32,
    pub mound_layer: u8,
}

#[derive(Debug, Clone)]
pub struct WindStreak {
    pub x: f32,
    pub y: f32,
    pub speed: f32,
    pub life: f32,
    pub max_life: f32,
    pub ch: char,
}

#[derive(Debug, Clone)]
pub struct GrassTuft {
    pub x: u16,
    pub y: u16,
    pub glyph: &'static str,
}

#[derive(Debug, Clone)]
pub struct TerrainProfile {
    pub width: u16,
    pub height: u16,
    pub ground_heights: Vec<u16>,
    pub mound_levels: Vec<f32>,
    pub grass_tufts: Vec<GrassTuft>,
    pub puddles: Vec<(u16, u16)>, // (start_x, end_x)
}

impl TerrainProfile {
    pub fn new(width: u16, height: u16) -> Self {
        let mut terrain = Self {
            width,
            height,
            ground_heights: Vec::new(),
            mound_levels: Vec::new(),
            grass_tufts: Vec::new(),
            puddles: Vec::new(),
        };
        terrain.recalculate();
        terrain
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.recalculate();
        }
    }

    pub fn recalculate(&mut self) {
        let width = self.width.max(1) as usize;
        let base_y = self.height.saturating_sub(2);
        let mut heights = Vec::with_capacity(width);
        let mounds = vec![0.0f32; width];
        let mut grass = Vec::new();
        let mut puddles = Vec::new();

        let w_f32 = width as f32;
        let tuft_patterns = [",,", "\\//", ";;", "..", "||", "~"];

        for x in 0..width {
            let fx = x as f32;
            let wave = (fx * 0.09).sin() * 0.55 + ((fx / w_f32) * std::f32::consts::PI * 3.2).cos() * 0.45;
            let offset = wave.round() as i32;
            let gy = (base_y as i32 + offset).clamp(base_y.saturating_sub(1) as i32, base_y as i32) as u16;
            heights.push(gy);

            // Pseudo-random deterministic placement of grass tufts
            let pseudo_hash = ((x as u32 * 2654435761) ^ (x as u32 >> 3)) % 100;
            if pseudo_hash < 14 && x > 2 && x < width.saturating_sub(3) {
                let p_idx = (pseudo_hash as usize) % tuft_patterns.len();
                grass.push(GrassTuft {
                    x: x as u16,
                    y: gy.saturating_sub(1),
                    glyph: tuft_patterns[p_idx],
                });
            }
        }

        // Detect low-lying depressions for puddles
        let mut in_puddle = false;
        let mut puddle_start = 0;
        for x in 0..width {
            if heights[x] >= base_y {
                if !in_puddle {
                    in_puddle = true;
                    puddle_start = x as u16;
                }
            } else if in_puddle {
                in_puddle = false;
                let len = x as u16 - puddle_start;
                if len >= 4 {
                    puddles.push((puddle_start, x as u16));
                }
            }
        }
        if in_puddle && width as u16 - puddle_start >= 4 {
            puddles.push((puddle_start, width as u16));
        }

        self.ground_heights = heights;
        self.mound_levels = mounds;
        self.grass_tufts = grass;
        self.puddles = puddles;
    }

    #[inline]
    pub fn get_ground_y(&self, x: u16) -> u16 {
        let default_y = self.height.saturating_sub(2);
        if self.ground_heights.is_empty() {
            return default_y;
        }
        let idx = (x as usize).min(self.ground_heights.len().saturating_sub(1));
        self.ground_heights.get(idx).copied().unwrap_or(default_y)
    }

    pub fn add_petal_to_mound(&mut self, x: u16) {
        if self.mound_levels.is_empty() {
            return;
        }
        let idx = (x as usize).min(self.mound_levels.len().saturating_sub(1));
        if let Some(m) = self.mound_levels.get_mut(idx) {
            *m = (*m + 0.35).min(3.0);
        }
    }

    pub fn tick(&mut self, gust_intensity: f32) {
        for m in &mut self.mound_levels {
            *m = (*m - 0.0015).max(0.0);
            if gust_intensity > 0.5 {
                *m = (*m - 0.008 * gust_intensity).max(0.0);
            }
        }
    }
}

pub struct ParticleEngine {
    pub leaves: Vec<Leaf>,
    pub settled: Vec<SettledBlossom>,
    pub wind_streaks: Vec<WindStreak>,
    pub terrain: TerrainProfile,
    pub width: u16,
    pub height: u16,
    pub speed: f32,
    pub sway: f32,
    pub sway_amplitude: f32,
    pub time: f32,
    pub gust_intensity: f32,
    pub gust_direction: f32,
    pub gust_timer: f32,
    pub gust_duration: f32,
    pub current_wind: f32,
    pub target_count: usize,
    pub feedback_msg: Option<(String, std::time::Instant)>,
}

impl ParticleEngine {
    pub fn new(tree_grid: &[Vec<char>], width: u16, height: u16, speed: f32, sway: f32) -> Self {
        let mut engine = Self {
            leaves: Vec::with_capacity(384),
            settled: Vec::with_capacity(256),
            wind_streaks: Vec::with_capacity(32),
            terrain: TerrainProfile::new(width, height),
            width,
            height,
            speed,
            sway,
            sway_amplitude: sway * 1.5,
            time: 0.0,
            gust_intensity: 0.0,
            gust_direction: 1.0,
            gust_timer: 6.0,
            gust_duration: 4.0,
            current_wind: 0.0,
            target_count: 150,
            feedback_msg: None,
        };
        engine.spawn_initial(tree_grid);
        engine
    }

    pub fn set_feedback(&mut self, text: impl Into<String>) {
        self.feedback_msg = Some((text.into(), std::time::Instant::now()));
    }

    pub fn get_active_feedback(&self) -> Option<&str> {
        if let Some((ref msg, instant)) = self.feedback_msg {
            if instant.elapsed() < std::time::Duration::from_millis(1600) {
                return Some(msg);
            }
        }
        None
    }

    pub fn trigger_gust(&mut self) {
        let mut rng = rand::thread_rng();
        self.gust_duration = rng.gen_range(4.0..7.0);
        self.gust_timer = self.gust_duration;
        self.gust_intensity = 2.4;
        self.gust_direction = if rng.gen_bool(0.85) { 1.0 } else { -1.0 };

        let streak_chars = ['~', '≈', '>', '»', '-'];
        for _ in 0..6 {
            let ch = streak_chars[rng.gen_range(0..streak_chars.len())];
            let start_x = if self.gust_direction >= 0.0 { 0.0 } else { self.width as f32 };
            self.wind_streaks.push(WindStreak {
                x: start_x,
                y: rng.gen_range(2.0..(self.height.saturating_sub(3) as f32).max(3.0)),
                speed: (self.gust_intensity * 3.2 + rng.gen_range(2.0..4.5)) * self.gust_direction,
                life: 0.0,
                max_life: rng.gen_range(1.2..2.2),
                ch,
            });
        }
        self.set_feedback("Wind Gust Surge Triggered");
    }

    pub fn cycle_sway(&mut self) {
        let (new_sway, label) = if self.sway < 0.8 {
            (1.0, "1.0x (Moderate Breeze)")
        } else if self.sway < 1.8 {
            (2.2, "2.2x (Blustery Gale)")
        } else if self.sway < 2.8 {
            (3.5, "3.5x (Stormy Gusts)")
        } else {
            (0.5, "0.5x (Calm Drift)")
        };
        self.sway = new_sway;
        self.sway_amplitude = new_sway * 1.5;
        self.set_feedback(format!("Wind Sway: {}", label));
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.terrain.resize(width, height);
        self.settled.retain(|s| s.x < width && s.y < height);
        self.wind_streaks.retain(|w| w.x < width as f32 && w.y < height as f32);
    }

    fn spawn_initial(&mut self, tree_grid: &[Vec<char>]) {
        let mut rng = rand::thread_rng();
        let target_count = self.target_count;
        for _ in 0..target_count {
            let mut p = Self::create_particle(self.width, self.height, self.speed, tree_grid, &mut rng);
            p.y = rng.gen_range(0.0..(self.height as f32).max(1.0));
            self.leaves.push(p);
        }
    }

    fn create_particle(width: u16, height: u16, speed: f32, _tree_grid: &[Vec<char>], rng: &mut impl Rng) -> Leaf {
        let chars = ['*', '+', '.', 'o', '%', '#', '·'];
        let base_ch = chars[rng.gen_range(0..chars.len())];
        let color_idx = rng.gen_range(0..4);
        let layer = if rng.gen_bool(0.45) {
            ParallaxLayer::Background
        } else {
            ParallaxLayer::Foreground
        };

        let spawn_x = rng.gen_range(0.0..(width as f32).max(1.0));
        let spawn_y = rng.gen_range(0.0..((height / 3) as f32).max(1.0));
        let base_speed = match layer {
            ParallaxLayer::Foreground => rng.gen_range(1.1..3.0),
            ParallaxLayer::Background => rng.gen_range(0.6..1.6),
        };

        Leaf {
            x: spawn_x,
            y: spawn_y,
            vx: rng.gen_range(-0.25..0.25),
            vy: rng.gen_range(0.2..0.75) * speed * if layer == ParallaxLayer::Background { 0.75 } else { 1.15 },
            phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
            speed: base_speed,
            state: if rng.gen_bool(0.3) { LeafState::Attached } else { LeafState::Falling },
            base_ch,
            color_idx,
            layer,
            angle: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
            angular_velocity: rng.gen_range(-0.15..0.15),
        }
    }

    pub fn tick(&mut self, tree_grid: &[Vec<char>]) {
        self.time += 0.033;
        if self.time > 100_000.0 {
            self.time = self.time.rem_euclid(std::f32::consts::PI * 200.0);
        }
        let mut rng = rand::thread_rng();
        let width = self.width;
        let height = self.height;
        let speed = self.speed;
        let sway = self.sway;

        // 1. Dynamic Wind Gust State Machine
        self.gust_timer -= 0.033;
        if self.gust_timer <= 0.0 {
            self.gust_duration = rng.gen_range(3.2..6.8);
            self.gust_timer = self.gust_duration + rng.gen_range(6.0..14.0);
            self.gust_direction = if rng.gen_bool(0.82) { 1.0 } else { -1.0 };
        }

        let active_gust = if self.gust_timer > 0.0 && self.gust_timer < self.gust_duration {
            let t = (self.gust_timer / self.gust_duration) * std::f32::consts::PI;
            t.sin() * 1.9 * sway.max(0.3)
        } else {
            0.0
        };

        // Smoothly interpolate wind gust strength
        self.gust_intensity = self.gust_intensity * 0.93 + active_gust * 0.07;

        // Base ambient breeze + Gust vector
        let base_breeze = (self.time * 0.65).sin() * 0.35 * sway;
        self.current_wind = base_breeze + (self.gust_intensity * self.gust_direction);

        // Update terrain dynamics (decay mounds, react to gusts)
        self.terrain.tick(self.gust_intensity);

        // 2. Wind Gust Streak Trails
        if self.gust_intensity > 0.35 && rng.gen_bool((self.gust_intensity * 0.4).min(0.7) as f64) && self.wind_streaks.len() < 24 {
            let streak_chars = ['~', '≈', '>', '»', '-'];
            let ch = streak_chars[rng.gen_range(0..streak_chars.len())];
            let start_x = if self.current_wind >= 0.0 { 0.0 } else { width as f32 };
            self.wind_streaks.push(WindStreak {
                x: start_x,
                y: rng.gen_range(2.0..(height.saturating_sub(3) as f32).max(3.0)),
                speed: (self.current_wind.abs() * 2.5 + rng.gen_range(1.5..3.5)) * self.current_wind.signum(),
                life: 0.0,
                max_life: rng.gen_range(0.8..1.8),
                ch,
            });
        }

        for streak in &mut self.wind_streaks {
            streak.life += 0.033;
            streak.x += streak.speed;
        }
        self.wind_streaks.retain(|s| s.life < s.max_life && s.x >= 0.0 && s.x < width as f32);

        // 3. Update Leaf Simulation (Parallax & Aerodynamic Tumbling)
        for p in &mut self.leaves {
            let layer_mult = match p.layer {
                ParallaxLayer::Foreground => 1.2,
                ParallaxLayer::Background => 0.65,
            };

            p.phase = (p.phase + (0.05 + self.gust_intensity * 0.06) * p.speed).rem_euclid(std::f32::consts::PI * 2.0);

            // Aerodynamic angular velocity updates for rotational tumbling
            let wind_torque = self.current_wind * 0.08 * layer_mult;
            p.angular_velocity += wind_torque + ((p.phase + self.time * 2.5).sin() * 0.035 * layer_mult);
            p.angular_velocity *= 0.94; // Angular damping
            p.angle = (p.angle + p.angular_velocity).rem_euclid(std::f32::consts::PI * 2.0);

            match p.state {
                LeafState::Attached => {
                    let detach_prob = (0.003 + (self.gust_intensity * 0.016)) * layer_mult;
                    if rng.gen_bool(detach_prob.min(0.2) as f64) {
                        p.state = LeafState::Falling;
                    }
                }
                LeafState::Falling => {
                    // 1. Spatial wind wave propagation across screen width
                    let spatial_wave = ((self.time * 2.2) - (p.x * 0.035)).sin() * (0.35 + self.gust_intensity * 0.45) * layer_mult;
                    let micro_turbulence = ((self.time * 3.8) + p.phase).cos() * 0.18 * (1.0 + self.gust_intensity) * layer_mult;

                    let total_wind_x = (self.current_wind * layer_mult) + spatial_wave + micro_turbulence;
                    p.x += p.vx + total_wind_x;

                    // 2. Aerodynamic Canopy Leeward Swirl Vortex
                    let canopy_cx = width as f32 * 0.5;
                    let canopy_cy = (height as f32 * 0.38).min(18.0);
                    let vortex_offset_x = (self.current_wind * 9.0).clamp(-14.0, 14.0);
                    let vortex_cx = canopy_cx + vortex_offset_x;
                    let vortex_cy = canopy_cy + ((self.time * 1.8).sin() * 1.5);
                    let vortex_radius = 12.0f32;

                    let v_dx = p.x - vortex_cx;
                    let v_dy = p.y - vortex_cy;
                    let v_dist_sq = v_dx * v_dx + v_dy * v_dy;

                    if v_dist_sq < vortex_radius * vortex_radius && v_dist_sq > 0.8 && self.current_wind.abs() > 0.15 {
                        let v_dist = v_dist_sq.sqrt();
                        let swirl_strength = (1.0 - (v_dist / vortex_radius)) * (self.current_wind.abs() * 0.28) * self.current_wind.signum() * layer_mult;
                        let tang_x = (-v_dy / v_dist) * swirl_strength;
                        let tang_y = (v_dx / v_dist) * swirl_strength;
                        p.x += tang_x;
                        p.y += tang_y;
                        p.angular_velocity += swirl_strength * 0.15;
                    }

                    // 3. Aerodynamic petal lift during strong wind gusts
                    let lift = (self.gust_intensity * 0.16 * ((self.time * 2.8 + p.phase).sin() + 0.4) * layer_mult).clamp(0.0, 0.35);
                    p.y += (p.vy - lift).max(0.06);

                    // 4. Branch Collision & Deflection (Full & Scaled Terminals)
                    let art_height = tree_grid.len();
                    let art_width = tree_grid.iter().map(|r| r.len()).max().unwrap_or(80);
                    let target_height = height.saturating_sub(1) as usize;

                    if p.y >= 0.0 && p.x >= 0.0 && art_height > 0 && art_width > 0 {
                        let (tree_r, tree_c) = if art_height <= target_height {
                            let tree_offset_x = (width as usize).saturating_sub(art_width) / 2;
                            let tree_offset_y = target_height - art_height;
                            let px_u = p.x.round() as usize;
                            let py_u = p.y.round() as usize;
                            if px_u >= tree_offset_x && py_u >= tree_offset_y {
                                (Some(py_u - tree_offset_y), Some(px_u - tree_offset_x))
                            } else {
                                (None, None)
                            }
                        } else {
                            let step_y = if target_height > 1 {
                                (art_height - 1) as f32 / (target_height - 1) as f32
                            } else {
                                1.0
                            };
                            let step_x = (art_width as f32 / width.max(1) as f32).max(1.0);
                            let scaled_width = (art_width as f32 / step_x).ceil() as usize;
                            let offset_x = (width as usize).saturating_sub(scaled_width) / 2;
                            let px_u = p.x.round() as usize;
                            let py_u = p.y.round() as usize;
                            if px_u >= offset_x && py_u < target_height {
                                let bx = px_u - offset_x;
                                let sy = ((py_u as f32 * step_y).round() as usize).min(art_height.saturating_sub(1));
                                let sx = (bx as f32 * step_x).round() as usize;
                                (Some(sy), Some(sx))
                            } else {
                                (None, None)
                            }
                        };

                        if let (Some(tr), Some(tc)) = (tree_r, tree_c) {
                            if tr < art_height && tc < tree_grid[tr].len() {
                                let branch_ch = tree_grid[tr][tc];
                                if matches!(branch_ch, '#' | '%' | '@' | '=') && rng.gen_bool(0.18) {
                                    p.vx = -p.vx * 0.7 + rng.gen_range(-0.35..0.35);
                                    p.vy = (p.vy * 0.55).max(0.08);
                                    p.angular_velocity += rng.gen_range(-0.35..0.35);
                                }
                            }
                        }
                    }

                    let target_ground_y = self.terrain.get_ground_y(p.x.round().clamp(0.0, width.saturating_sub(1) as f32) as u16);

                    if p.y >= target_ground_y as f32 {
                        let gx = (p.x.round() as u16).min(width.saturating_sub(1));
                        let mound_h = self.terrain.mound_levels.get(gx as usize).copied().unwrap_or(0.0);
                        let mound_layer = if mound_h >= 1.8 { 2 } else if mound_h >= 0.8 { 1 } else { 0 };

                        let gy = target_ground_y.saturating_sub(mound_layer as u16);

                        if self.settled.len() < 180 {
                            self.settled.push(SettledBlossom {
                                x: gx,
                                y: gy,
                                ch: p.current_glyph(),
                                color_idx: p.color_idx,
                                alpha: 1.0,
                                decay_rate: rng.gen_range(0.003..0.006),
                                mound_layer,
                            });
                            self.terrain.add_petal_to_mound(gx);
                        }

                        *p = Self::create_particle(width, height, speed, tree_grid, &mut rng);
                    }

                    if p.x < 0.0 {
                        p.x = (width.saturating_sub(1)) as f32;
                    } else if p.x >= width as f32 {
                        p.x = 0.0;
                    }
                }
                LeafState::Settled | LeafState::Fading => {
                    *p = Self::create_particle(width, height, speed, tree_grid, &mut rng);
                }
            }
        }

        // 4. Update Settled Ground Petals (with organic terrain rustle and gust scattering)
        let ground_rustle_chance = 0.015 + (self.gust_intensity * 0.07);
        for s in &mut self.settled {
            s.alpha -= s.decay_rate;
            if rng.gen_bool(ground_rustle_chance.min(0.3) as f64) {
                let shift = if self.gust_direction >= 0.0 {
                    if rng.gen_bool(0.78) { 1 } else { -1 }
                } else {
                    if rng.gen_bool(0.78) { -1 } else { 1 }
                };
                let new_x = (s.x as i32 + shift).clamp(0, width.saturating_sub(1) as i32) as u16;
                s.x = new_x;
                s.y = self.terrain.get_ground_y(new_x).saturating_sub(s.mound_layer as u16);
            }
        }
        self.settled.retain(|s| s.alpha > 0.0);

        while self.leaves.len() < self.target_count {
            self.leaves.push(Self::create_particle(width, height, speed, tree_grid, &mut rng));
        }
    }
}
