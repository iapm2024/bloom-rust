use rand::Rng;

#[derive(Debug, Clone)]
pub struct Star {
    pub x: u16,
    pub y: u16,
    pub phase: f32,
    pub speed: f32,
}

pub struct StarrySky {
    pub stars: Vec<Star>,
    pub width: u16,
    pub height: u16,
}

impl StarrySky {
    pub fn new(width: u16, height: u16) -> Self {
        let mut sky = Self {
            stars: Vec::new(),
            width,
            height,
        };
        sky.generate_stars();
        sky
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            self.generate_stars();
        }
    }

    fn generate_stars(&mut self) {
        self.stars.clear();
        let mut rng = rand::thread_rng();
        let count = ((self.width as u32 * self.height as u32) / 40).clamp(20, 150) as usize;

        for _ in 0..count {
            let x = rng.gen_range(0..self.width.max(1));
            let y = rng.gen_range(0..self.height.saturating_sub(2).max(1));
            let phase = rng.gen_range(0.0..std::f32::consts::PI * 2.0);
            let speed = rng.gen_range(0.02..0.08);

            self.stars.push(Star { x, y, phase, speed });
        }
    }

    pub fn tick(&mut self) {
        for star in &mut self.stars {
            star.phase += star.speed;
            if star.phase > std::f32::consts::PI * 2.0 {
                star.phase -= std::f32::consts::PI * 2.0;
            }
        }
    }
}
