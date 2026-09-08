use rand::Rng;
use crate::config::{Mood, Season};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BirdFlightState {
    FlyingIn,
    Perched,
    FlyingOut,
}

#[derive(Debug, Clone)]
pub struct Bird {
    pub x: f32,
    pub y: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub branch_r: usize,
    pub branch_c: usize,
    pub vx: f32,
    pub vy: f32,
    pub state: BirdFlightState,
    pub perch_timer: f32,
    pub facing_right: bool,
    pub flap_timer: f32,
    pub chirp_timer: f32,
    pub is_chirping: bool,
}

#[derive(Debug, Clone)]
pub struct Firefly {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub pulse_phase: f32,
    pub pulse_speed: f32,
    pub wander_angle: f32,
    pub wander_speed: f32,
}

#[derive(Debug, Clone)]
pub struct FloatingPetal {
    pub x: f32,
    pub y: u16,
    pub puddle_start: u16,
    pub puddle_end: u16,
    pub ch: char,
    pub color_idx: usize,
    pub vx: f32,
    pub bob_phase: f32,
    pub life: f32,
    pub max_life: f32,
}

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
            let pseudo_hash = ((x as u32).wrapping_mul(2654435761) ^ (x as u32 >> 3)) % 100;
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
    pub birds: Vec<Bird>,
    pub fireflies: Vec<Firefly>,
    pub floating_petals: Vec<FloatingPetal>,
    pub bird_spawn_timer: f32,
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
    pub art_height: usize,
    pub art_width: usize,
    pub cached_branch_perches: Vec<(usize, usize)>,
}

impl ParticleEngine {
    pub fn new(tree_grid: &[Vec<char>], width: u16, height: u16, speed: f32, sway: f32) -> Self {
        let art_height = tree_grid.len();
        let art_width = tree_grid.iter().map(|r| r.len()).max().unwrap_or(80);

        let mut cached_branch_perches = Vec::new();
        if art_height > 10 && art_width > 10 {
            let min_r = (art_height * 30) / 100;
            let max_r = (art_height * 80) / 100;
            for r in min_r..max_r {
                if r < art_height {
                    let row = &tree_grid[r];
                    for (c, &ch) in row.iter().enumerate() {
                        if matches!(ch, '#' | '%' | '@' | '=') {
                            let space_above = if r > 0 && c < tree_grid[r - 1].len() {
                                matches!(tree_grid[r - 1][c], ' ' | '*' | '+' | '.' | '~')
                            } else {
                                true
                            };
                            if space_above {
                                cached_branch_perches.push((r, c));
                            }
                        }
                    }
                }
            }
        }

        let mut engine = Self {
            leaves: Vec::with_capacity(384),
            settled: Vec::with_capacity(256),
            wind_streaks: Vec::with_capacity(32),
            terrain: TerrainProfile::new(width, height),
            birds: Vec::with_capacity(4),
            fireflies: Vec::with_capacity(32),
            floating_petals: Vec::with_capacity(48),
            bird_spawn_timer: 6.0,
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
            art_height,
            art_width,
            cached_branch_perches,
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

        // Startle any perched birds into flight
        for bird in &mut self.birds {
            if bird.state == BirdFlightState::Perched {
                bird.state = BirdFlightState::FlyingOut;
                bird.is_chirping = false;
                bird.vy = -0.7;
                bird.vx = if self.gust_direction >= 0.0 { 1.3 } else { -1.3 };
                bird.facing_right = bird.vx >= 0.0;
            }
        }

        self.set_feedback("Wind Gust Surge Triggered");
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.terrain.resize(width, height);
        let max_w = width.saturating_sub(1) as f32;
        let max_h = height.saturating_sub(1) as f32;
        for p in &mut self.leaves {
            p.x = p.x.clamp(0.0, max_w);
            p.y = p.y.clamp(0.0, max_h);
        }
        self.settled.retain(|s| s.x < width && s.y < height);
        self.wind_streaks.retain(|w| w.x < max_w && w.y < max_h);
        self.floating_petals.retain(|fp| (fp.x.round() as u16) < width && fp.y < height);
        self.fireflies.retain(|ff| (ff.x.round() as u16) < width && (ff.y.round() as u16) < height);
        self.birds.retain(|b| b.x >= -12.0 && b.x <= (width + 12) as f32 && b.y >= -6.0 && b.y <= height as f32);
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
        let color_idx = rng.gen_range(0..8);
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

    pub fn tick(&mut self, tree_grid: &[Vec<char>], season: Season, mood: Mood, is_raining: bool) {
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
        let canopy_cx = width as f32 * 0.5;
        let canopy_cy = (height as f32 * 0.38).min(18.0);
        let vortex_offset_x = (self.current_wind * 9.0).clamp(-14.0, 14.0);
        let vortex_cx = canopy_cx + vortex_offset_x;
        let vortex_cy = canopy_cy + ((self.time * 1.8).sin() * 1.5);
        let vortex_radius = 12.0f32;
        let vortex_radius_sq = vortex_radius * vortex_radius;

        let art_height = self.art_height;
        let art_width = self.art_width;
        let target_height = height.saturating_sub(1) as usize;
        let is_1to1 = art_height <= target_height;
        let tree_offset_x = (width as usize).saturating_sub(art_width) / 2;
        let tree_offset_y = target_height.saturating_sub(art_height);

        let step_y = if target_height > 1 && art_height > 1 {
            ((art_height - 1) as f32 / (target_height - 1) as f32).max(0.001)
        } else {
            1.0
        };
        let step_x = (art_width as f32 / width.max(1) as f32).max(1.0);
        let scaled_width = (art_width as f32 / step_x).ceil() as usize;
        let scaled_offset_x = (width as usize).saturating_sub(scaled_width) / 2;

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
                    let v_dx = p.x - vortex_cx;
                    let v_dy = p.y - vortex_cy;
                    let v_dist_sq = v_dx * v_dx + v_dy * v_dy;

                    if v_dist_sq < vortex_radius_sq && v_dist_sq > 0.8 && self.current_wind.abs() > 0.15 {
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
                    if p.y >= 0.0 && p.x >= 0.0 && art_height > 0 && art_width > 0 {
                        let px_u = p.x.round() as usize;
                        let py_u = p.y.round() as usize;
                        let (tree_r, tree_c) = if is_1to1 {
                            if px_u >= tree_offset_x && py_u >= tree_offset_y {
                                (Some(py_u - tree_offset_y), Some(px_u - tree_offset_x))
                            } else {
                                (None, None)
                            }
                        } else if px_u >= scaled_offset_x && py_u < target_height {
                            let bx = px_u - scaled_offset_x;
                            let sy = ((py_u as f32 * step_y).round() as usize).min(art_height.saturating_sub(1));
                            let sx = (bx as f32 * step_x).round() as usize;
                            (Some(sy), Some(sx))
                        } else {
                            (None, None)
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
                        let in_puddle = if is_raining {
                            self.terrain.puddles.iter().find(|&&(s, e)| gx >= s && gx < e).copied()
                        } else {
                            None
                        };

                        if let Some((p_start, p_end)) = in_puddle {
                            if self.floating_petals.len() < 40 {
                                self.floating_petals.push(FloatingPetal {
                                    x: gx as f32,
                                    y: target_ground_y,
                                    puddle_start: p_start,
                                    puddle_end: p_end,
                                    ch: p.current_glyph(),
                                    color_idx: p.color_idx,
                                    vx: rng.gen_range(-0.06..0.06) + self.current_wind * 0.04,
                                    bob_phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
                                    life: 0.0,
                                    max_life: rng.gen_range(8.0..16.0),
                                });
                            }
                            *p = Self::create_particle(width, height, speed, tree_grid, &mut rng);
                        } else {
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

        // 5. Update Floating Petals in Rain Puddles
        let decay_mult = if is_raining { 1.0 } else { 3.5 };
        for fp in &mut self.floating_petals {
            fp.life += 0.033 * decay_mult;
            fp.bob_phase = (fp.bob_phase + 0.1).rem_euclid(std::f32::consts::PI * 2.0);
            let water_drift = (self.current_wind * 0.07) + (self.time * 2.2 + fp.bob_phase).sin() * 0.04;
            fp.x += fp.vx + water_drift;
            if fp.x < fp.puddle_start as f32 {
                fp.x = fp.puddle_start as f32;
                fp.vx = fp.vx.abs();
            } else if fp.x >= fp.puddle_end as f32 {
                fp.x = (fp.puddle_end.saturating_sub(1)) as f32;
                fp.vx = -fp.vx.abs();
            }
        }
        self.floating_petals.retain(|fp| fp.life < fp.max_life);

        // 6. Update Summer Night Fireflies (Hotaru)
        let is_summer_night = season == Season::Summer && mood == Mood::Night;
        if is_summer_night {
            if self.fireflies.len() < 22 && rng.gen_bool(0.25) {
                let min_fx = 2.0f32;
                let max_fx = (width.saturating_sub(2) as f32).max(min_fx + 1.0);
                let fx = rng.gen_range(min_fx..max_fx);
                let gy = self.terrain.get_ground_y(fx as u16) as f32;
                let min_fy = 2.0f32;
                let max_fy = gy.max(min_fy + 1.0);
                let fy = rng.gen_range(min_fy..max_fy);
                self.fireflies.push(Firefly {
                    x: fx,
                    y: fy,
                    vx: rng.gen_range(-0.15..0.15),
                    vy: rng.gen_range(-0.1..0.1),
                    pulse_phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
                    pulse_speed: rng.gen_range(0.04..0.09),
                    wander_angle: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
                    wander_speed: rng.gen_range(0.12..0.28),
                });
            }
        } else if !self.fireflies.is_empty() {
            self.fireflies.pop();
        }

        for ff in &mut self.fireflies {
            ff.pulse_phase = (ff.pulse_phase + ff.pulse_speed).rem_euclid(std::f32::consts::PI * 2.0);
            ff.wander_angle += rng.gen_range(-0.3..0.3);
            ff.vx = ff.vx * 0.92 + ff.wander_angle.cos() * ff.wander_speed * 0.08;
            ff.vy = ff.vy * 0.92 + (ff.wander_angle.sin() * ff.wander_speed * 0.5) * 0.08;
            ff.x += ff.vx + self.current_wind * 0.06;
            ff.y += ff.vy;

            let max_w = (width.saturating_sub(2) as f32).max(2.0);
            let max_h = (height.saturating_sub(3) as f32).max(3.0);
            if ff.x < 1.0 { ff.x = 1.0; ff.vx = ff.vx.abs(); }
            if ff.x > max_w { ff.x = max_w; ff.vx = -ff.vx.abs(); }
            if ff.y < 2.0 { ff.y = 2.0; ff.vy = ff.vy.abs(); }
            if ff.y > max_h { ff.y = max_h; ff.vy = -ff.vy.abs(); }
        }

        // 7. Update Perching Birds (Japanese White-Eye / Sparrow)
        self.bird_spawn_timer -= 0.033;
        if self.bird_spawn_timer <= 0.0 && self.birds.len() < 2 && !self.cached_branch_perches.is_empty() {
            self.bird_spawn_timer = rng.gen_range(16.0..32.0);
            let &(target_r, target_c) = &self.cached_branch_perches[rng.gen_range(0..self.cached_branch_perches.len())];
            let (target_x, target_y) = if is_1to1 {
                ((tree_offset_x + target_c) as f32, (tree_offset_y + target_r).saturating_sub(1) as f32)
            } else {
                let sc = (target_c as f32 / step_x).round() as usize;
                let sr = ((target_r as f32 / step_y).round() as usize).saturating_sub(1);
                ((scaled_offset_x + sc) as f32, sr as f32)
            };

            let from_left = rng.gen_bool(0.5);
            let spawn_x = if from_left { -4.0 } else { (width + 4) as f32 };
            let min_y = 2.0f32;
            let max_y = target_y.max(min_y + 1.0);
            let spawn_y = rng.gen_range(min_y..max_y);
            let dx = target_x - spawn_x;
            let dy = target_y - spawn_y;
            let dist = (dx * dx + dy * dy).sqrt().max(1.0);
            let speed = rng.gen_range(0.65..0.95);

            self.birds.push(Bird {
                x: spawn_x,
                y: spawn_y,
                target_x,
                target_y,
                branch_r: target_r,
                branch_c: target_c,
                vx: (dx / dist) * speed,
                vy: (dy / dist) * speed,
                state: BirdFlightState::FlyingIn,
                perch_timer: rng.gen_range(7.0..16.0),
                facing_right: dx >= 0.0,
                flap_timer: 0.0,
                chirp_timer: rng.gen_range(2.0..4.0),
                is_chirping: false,
            });
        }

        for bird in &mut self.birds {
            match bird.state {
                BirdFlightState::FlyingIn => {
                    bird.flap_timer += 0.033;
                    let dx = bird.target_x - bird.x;
                    let dy = bird.target_y - bird.y;
                    let dist = (dx * dx + dy * dy).sqrt();
                    if dist < 1.2 {
                        bird.x = bird.target_x;
                        bird.y = bird.target_y;
                        bird.state = BirdFlightState::Perched;
                        bird.vx = 0.0;
                        bird.vy = 0.0;
                    } else {
                        bird.facing_right = dx >= 0.0;
                        let fly_speed = 0.75;
                        bird.vx = (dx / dist) * fly_speed;
                        bird.vy = (dy / dist) * fly_speed;
                        bird.x += bird.vx;
                        bird.y += bird.vy;
                    }
                }
                BirdFlightState::Perched => {
                    bird.perch_timer -= 0.033;
                    bird.chirp_timer -= 0.033;
                    if bird.chirp_timer <= 0.0 {
                        bird.is_chirping = !bird.is_chirping;
                        bird.chirp_timer = if bird.is_chirping {
                            rng.gen_range(0.8..1.5)
                        } else {
                            rng.gen_range(2.5..5.5)
                        };
                    }
                    if rng.gen_bool(0.015) {
                        bird.facing_right = !bird.facing_right;
                    }
                    if self.gust_intensity > 0.75 {
                        bird.state = BirdFlightState::FlyingOut;
                        bird.is_chirping = false;
                        bird.vy = rng.gen_range(-0.9..-0.6);
                        bird.vx = if self.gust_direction >= 0.0 { rng.gen_range(0.8..1.5) } else { rng.gen_range(-1.5..-0.8) };
                        bird.facing_right = bird.vx >= 0.0;
                    } else if bird.perch_timer <= 0.0 {
                        bird.state = BirdFlightState::FlyingOut;
                        bird.is_chirping = false;
                        bird.vy = rng.gen_range(-0.7..-0.4);
                        bird.vx = if bird.facing_right { rng.gen_range(0.6..1.2) } else { rng.gen_range(-1.2..-0.6) };
                    }
                }
                BirdFlightState::FlyingOut => {
                    bird.flap_timer += 0.033;
                    bird.x += bird.vx + self.current_wind * 0.05;
                    bird.y += bird.vy;
                    bird.facing_right = bird.vx >= 0.0;
                }
            }
        }
        self.birds.retain(|b| b.x >= -12.0 && b.x <= (width + 12) as f32 && b.y >= -6.0 && b.y <= height as f32);

        while self.leaves.len() < self.target_count {
            self.leaves.push(Self::create_particle(width, height, speed, tree_grid, &mut rng));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terrain_profile_dimensions() {
        let terrain = TerrainProfile::new(80, 24);
        assert_eq!(terrain.ground_heights.len(), 80);
        assert_eq!(terrain.mound_levels.len(), 80);
        let ground_y = terrain.get_ground_y(40);
        assert!(ground_y <= 24);
    }

    #[test]
    fn test_terrain_mound_accumulation() {
        let mut terrain = TerrainProfile::new(80, 24);
        let initial_mound = terrain.mound_levels[10];
        terrain.add_petal_to_mound(10);
        assert!(terrain.mound_levels[10] > initial_mound);
    }

    #[test]
    fn test_bird_perch_and_startle() {
        let bird = Bird {
            x: 40.0,
            y: 10.0,
            target_x: 40.0,
            target_y: 10.0,
            branch_r: 15,
            branch_c: 40,
            vx: 0.0,
            vy: 0.0,
            state: BirdFlightState::Perched,
            perch_timer: 10.0,
            facing_right: true,
            flap_timer: 0.0,
            chirp_timer: 2.0,
            is_chirping: false,
        };
        assert_eq!(bird.state, BirdFlightState::Perched);

        // Simulate a gust startle
        let grid = vec![vec!['#'; 80]; 24];
        let mut engine = ParticleEngine::new(&grid, 80, 24, 1.0, 1.0);
        engine.birds.push(bird);
        engine.trigger_gust();
        assert_eq!(engine.birds[0].state, BirdFlightState::FlyingOut);
        assert!(engine.birds[0].vy < 0.0); // Flying upward
    }

    #[test]
    fn test_fireflies_pulse_and_wander() {
        let mut ff = Firefly {
            x: 30.0,
            y: 12.0,
            vx: 0.1,
            vy: -0.05,
            pulse_phase: 0.0,
            pulse_speed: 0.05,
            wander_angle: 0.0,
            wander_speed: 0.2,
        };
        let initial_phase = ff.pulse_phase;
        ff.pulse_phase += ff.pulse_speed;
        assert!(ff.pulse_phase > initial_phase);
    }

    #[test]
    fn test_floating_petals_puddle_drift() {
        let mut fp = FloatingPetal {
            x: 15.0,
            y: 22,
            puddle_start: 10,
            puddle_end: 25,
            ch: '*',
            color_idx: 0,
            vx: 0.5,
            bob_phase: 0.0,
            life: 0.0,
            max_life: 10.0,
        };
        fp.x += fp.vx;
        assert!(fp.x >= 10.0 && fp.x <= 25.0);
    }

    #[test]
    fn test_summer_night_firefly_spawning_and_tick() {
        let grid = vec![vec!['#'; 80]; 24];
        let mut engine = ParticleEngine::new(&grid, 80, 24, 1.0, 1.0);
        // Run multiple ticks in Summer Night mode to trigger firefly spawning and movement
        for _ in 0..100 {
            engine.tick(&grid, Season::Summer, Mood::Night, false);
        }
        // Fireflies should have spawned without panicking
        assert!(!engine.fireflies.is_empty());
    }

    #[test]
    fn test_bird_expiry_and_flight_out_no_panic() {
        let grid = vec![vec!['#'; 80]; 24];
        let mut engine = ParticleEngine::new(&grid, 80, 24, 1.0, 1.0);
        let bird = Bird {
            x: 40.0,
            y: 10.0,
            target_x: 40.0,
            target_y: 10.0,
            branch_r: 15,
            branch_c: 40,
            vx: 0.0,
            vy: 0.0,
            state: BirdFlightState::Perched,
            perch_timer: 0.01, // Near expiry
            facing_right: true,
            flap_timer: 0.0,
            chirp_timer: 2.0,
            is_chirping: false,
        };
        engine.birds.push(bird);
        // Ticking should transition bird to FlyingOut with negative vy without panicking
        engine.tick(&grid, Season::Spring, Mood::Day, false);
        assert_eq!(engine.birds[0].state, BirdFlightState::FlyingOut);
        assert!(engine.birds[0].vy < 0.0);
    }
}
