use rand::Rng;

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
    pub ch: char,
    pub color_idx: usize,
}

#[derive(Debug, Clone)]
pub struct SettledBlossom {
    pub x: u16,
    pub y: u16,
    pub ch: char,
    pub color_idx: usize,
    pub alpha: f32,
    pub decay_rate: f32,
}

pub struct ParticleEngine {
    pub leaves: Vec<Leaf>,
    pub settled: Vec<SettledBlossom>,
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
}

impl ParticleEngine {
    pub fn new(tree_grid: &[Vec<char>], width: u16, height: u16, speed: f32, sway: f32) -> Self {
        let mut engine = Self {
            leaves: Vec::with_capacity(256),
            settled: Vec::with_capacity(256),
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
        };
        engine.spawn_initial(tree_grid);
        engine
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.settled.retain(|s| s.x < width && s.y < height);
    }

    fn spawn_initial(&mut self, _tree_grid: &[Vec<char>]) {
        let mut rng = rand::thread_rng();
        let target_count = 120;
        for _ in 0..target_count {
            let mut p = Self::create_particle(self.width, self.height, self.speed, _tree_grid, &mut rng);
            p.y = rng.gen_range(0.0..(self.height as f32).max(1.0));
            self.leaves.push(p);
        }
    }

    fn create_particle(width: u16, height: u16, speed: f32, _tree_grid: &[Vec<char>], rng: &mut impl Rng) -> Leaf {
        let chars = ['*', '+', '.', 'o', '%', '#'];
        let ch = chars[rng.gen_range(0..chars.len())];
        let color_idx = rng.gen_range(0..4);

        let spawn_x = rng.gen_range(0.0..(width as f32).max(1.0));
        let spawn_y = rng.gen_range(0.0..((height / 3) as f32).max(1.0));

        Leaf {
            x: spawn_x,
            y: spawn_y,
            vx: rng.gen_range(-0.3..0.3),
            vy: rng.gen_range(0.2..0.8) * speed,
            phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
            speed: rng.gen_range(1.0..3.0),
            state: if rng.gen_bool(0.3) { LeafState::Attached } else { LeafState::Falling },
            ch,
            color_idx,
        }
    }

    pub fn tick(&mut self, tree_grid: &[Vec<char>]) {
        self.time += 0.033;
        let mut rng = rand::thread_rng();
        let width = self.width;
        let height = self.height;
        let speed = self.speed;
        let sway = self.sway;
        let ground_row = height.saturating_sub(2);

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

        // 2. Update Leaf Simulation
        for p in &mut self.leaves {
            p.phase += (0.05 + self.gust_intensity * 0.06) * p.speed;
            match p.state {
                LeafState::Attached => {
                    let detach_prob = 0.004 + (self.gust_intensity * 0.018);
                    if rng.gen_bool(detach_prob.min(0.2) as f64) {
                        p.state = LeafState::Falling;
                    }
                }
                LeafState::Falling => {
                    // Spatial wind wave propagation across screen width
                    let spatial_wave = ((self.time * 2.2) - (p.x * 0.035)).sin() * (0.35 + self.gust_intensity * 0.45);
                    let micro_turbulence = ((self.time * 3.8) + p.phase).cos() * 0.18 * (1.0 + self.gust_intensity);

                    let total_wind_x = self.current_wind + spatial_wave + micro_turbulence;
                    p.x += p.vx + total_wind_x;

                    // Aerodynamic petal lift during strong wind gusts
                    let lift = (self.gust_intensity * 0.16 * ((self.time * 2.8 + p.phase).sin() + 0.4)).clamp(0.0, 0.35);
                    p.y += (p.vy - lift).max(0.08);

                    if p.y >= ground_row as f32 {
                        let gx = (p.x.round() as u16).min(width.saturating_sub(1));
                        let gy = if rng.gen_bool(0.18) && ground_row > 1 {
                            ground_row.saturating_sub(1)
                        } else {
                            ground_row
                        };

                        if self.settled.len() < 160 {
                            self.settled.push(SettledBlossom {
                                x: gx,
                                y: gy,
                                ch: p.ch,
                                color_idx: p.color_idx,
                                alpha: 1.0,
                                decay_rate: rng.gen_range(0.003..0.007),
                            });
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

        // 3. Update Settled Ground Petals (with gust rustle sensitivity)
        let ground_rustle_chance = 0.015 + (self.gust_intensity * 0.06);
        for s in &mut self.settled {
            s.alpha -= s.decay_rate;
            if rng.gen_bool(ground_rustle_chance.min(0.25) as f64) {
                let shift = if self.gust_direction >= 0.0 {
                    if rng.gen_bool(0.75) { 1 } else { -1 }
                } else {
                    if rng.gen_bool(0.75) { -1 } else { 1 }
                };
                let new_x = (s.x as i32 + shift).clamp(0, width.saturating_sub(1) as i32) as u16;
                s.x = new_x;
            }
        }
        self.settled.retain(|s| s.alpha > 0.0);

        while self.leaves.len() < 120 {
            self.leaves.push(Self::create_particle(width, height, speed, tree_grid, &mut rng));
        }
    }
}
