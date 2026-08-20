use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use std::process::Command;

const DEFAULT_CITY_NAME: &str = "Concepción";

#[derive(Debug, Clone)]
pub struct GnomeLocation {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherCondition {
    Clear,
    Cloudy,
    Fog,
    Rain,
    Snow,
}

#[derive(Debug, Clone)]
pub struct WeatherData {
    pub temp: f64,
    pub weathercode: u8,
}

pub struct CacheState {
    pub gnome_loc: Option<GnomeLocation>,
    pub ip_city: Option<String>,
    pub primary_data: Option<WeatherData>,
    pub santiago_data: Option<WeatherData>,
    pub last_fetch: Option<std::time::Instant>,
    pub is_fetching: bool,
    pub cached_condition: WeatherCondition,
    pub cached_info: String,
}

#[derive(Clone)]
pub struct WeatherFetcher {
    cache: Arc<Mutex<CacheState>>,
    enabled: bool,
    manual_city: String,
}

impl WeatherFetcher {
    pub fn new(enabled: bool, manual_city: String) -> Self {
        let default_info = if enabled {
            "Concepción · Updating...  │  Santiago · Updating...".to_string()
        } else {
            "Offline Mode".to_string()
        };

        let fetcher = Self {
            cache: Arc::new(Mutex::new(CacheState {
                gnome_loc: None,
                ip_city: None,
                primary_data: None,
                santiago_data: None,
                last_fetch: None,
                is_fetching: false,
                cached_condition: WeatherCondition::Clear,
                cached_info: default_info,
            })),
            enabled,
            manual_city,
        };

        if fetcher.enabled {
            fetcher.trigger_fetch_background();
        }

        fetcher
    }

    pub fn trigger_fetch_background(&self) {
        if !self.enabled {
            return;
        }

        let cache_clone = Arc::clone(&self.cache);
        let manual_city = self.manual_city.clone();

        {
            if let Ok(mut guard) = cache_clone.lock() {
                if guard.is_fetching {
                    return;
                }
                guard.is_fetching = true;
            } else {
                return;
            }
        }

        thread::spawn(move || {
            let client = reqwest::blocking::Client::builder()
                .timeout(Duration::from_secs(4))
                .user_agent("bloom-rust/0.3")
                .build()
                .ok();

            let gnome_loc = Self::detect_gnome_location();
            let mut detected_ip_city = None;
            let (req_lat, req_lon) = if manual_city.is_empty() {
                if let Some(ref loc) = gnome_loc {
                    (loc.lat, loc.lon)
                } else {
                    let ip_loc = Self::fetch_ip_location(client.as_ref());
                    detected_ip_city = Some(ip_loc.0);
                    (ip_loc.1, ip_loc.2)
                }
            } else {
                Self::geocode_city(&manual_city, client.as_ref()).unwrap_or((-36.8335, -73.0487))
            };

            let primary = Self::fetch_open_meteo(req_lat, req_lon, client.as_ref());
            let santiago = Self::fetch_open_meteo(-33.4489, -70.6693, client.as_ref());

            if let Ok(mut guard) = cache_clone.lock() {
                if gnome_loc.is_some() {
                    guard.gnome_loc = gnome_loc;
                }
                if detected_ip_city.is_some() {
                    guard.ip_city = detected_ip_city;
                }
                if primary.is_some() {
                    guard.primary_data = primary.clone();
                }
                if santiago.is_some() {
                    guard.santiago_data = santiago;
                }

                let condition = match guard.primary_data.as_ref().map(|d| d.weathercode) {
                    Some(0 | 1) => WeatherCondition::Clear,
                    Some(2 | 3) => WeatherCondition::Cloudy,
                    Some(45 | 48) => WeatherCondition::Fog,
                    Some(51..=67 | 80..=82 | 95..=99) => WeatherCondition::Rain,
                    Some(71..=77 | 85..=86) => WeatherCondition::Snow,
                    _ => WeatherCondition::Clear,
                };
                guard.cached_condition = condition;

                let city = if !manual_city.is_empty() {
                    manual_city.clone()
                } else if let Some(ref g) = guard.gnome_loc {
                    g.name.clone()
                } else if let Some(ref ip) = guard.ip_city {
                    ip.clone()
                } else {
                    DEFAULT_CITY_NAME.to_string()
                };

                let p_str = match guard.primary_data {
                    Some(ref d) => format!("{:.1}°C", d.temp),
                    None => "Updating...".to_string(),
                };

                let s_str = match guard.santiago_data {
                    Some(ref d) => format!("{:.1}°C", d.temp),
                    None => "Updating...".to_string(),
                };

                guard.cached_info = format!("{} · {}  │  Santiago · {}", city, p_str, s_str);
                guard.last_fetch = Some(std::time::Instant::now());
                guard.is_fetching = false;
            }
        });
    }

    fn geocode_city(city: &str, client: Option<&reqwest::blocking::Client>) -> Option<(f64, f64)> {
        let client = client?;
        let encoded_city = city.replace(' ', "%20");
        let url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1",
            encoded_city
        );

        let resp = client.get(&url).send().ok()?;
        let json: serde_json::Value = resp.json().ok()?;
        let lat = json["results"][0]["latitude"].as_f64()?;
        let lon = json["results"][0]["longitude"].as_f64()?;
        Some((lat, lon))
    }

    fn detect_gnome_location() -> Option<GnomeLocation> {
        let output = Command::new("gsettings")
            .args(["get", "org.gnome.Weather", "locations"])
            .output()
            .ok()?;

        let out_str = String::from_utf8_lossy(&output.stdout);
        if out_str.is_empty() || out_str.trim() == "@av []" {
            return None;
        }

        let city_name = Self::extract_city_name(&out_str).unwrap_or_else(|| DEFAULT_CITY_NAME.to_string());
        let (lat, lon) = Self::extract_coordinates(&out_str).unwrap_or((-36.8335, -73.0487));

        Some(GnomeLocation {
            name: city_name,
            lat,
            lon,
        })
    }

    fn extract_city_name(out: &str) -> Option<String> {
        let parts: Vec<&str> = out.split('\'').collect();
        if parts.len() >= 2 {
            Some(parts[1].trim().to_string())
        } else {
            None
        }
    }

    fn extract_coordinates(out: &str) -> Option<(f64, f64)> {
        let p1 = out.find('(')?;
        let p2 = out.find(')')?;
        if p2 <= p1 {
            return None;
        }

        let coord_str = &out[p1 + 1..p2];
        let parts: Vec<&str> = coord_str.split(',').collect();
        if parts.len() >= 2 {
            let lat_rad: f64 = parts[0].trim().parse().ok()?;
            let lon_rad: f64 = parts[1].trim().parse().ok()?;
            let lat_deg = lat_rad * (180.0 / std::f64::consts::PI);
            let lon_deg = lon_rad * (180.0 / std::f64::consts::PI);
            Some((lat_deg, lon_deg))
        } else {
            None
        }
    }

    fn fetch_ip_location(client: Option<&reqwest::blocking::Client>) -> (String, f64, f64) {
        if let Some(c) = client {
            if let Ok(resp) = c.get("https://ip-api.com/json/").send() {
                if let Ok(json) = resp.json::<serde_json::Value>() {
                    let city = json["city"].as_str().unwrap_or(DEFAULT_CITY_NAME).to_string();
                    let lat = json["lat"].as_f64().unwrap_or(-36.8335);
                    let lon = json["lon"].as_f64().unwrap_or(-73.0487);
                    return (city, lat, lon);
                }
            }
        }
        (DEFAULT_CITY_NAME.to_string(), -36.8335, -73.0487)
    }

    fn fetch_open_meteo(lat: f64, lon: f64, client: Option<&reqwest::blocking::Client>) -> Option<WeatherData> {
        let client = client?;
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={:.4}&longitude={:.4}&current_weather=true",
            lat, lon
        );

        let resp = client.get(&url).send().ok()?;
        let json: serde_json::Value = resp.json().ok()?;
        let temp = json["current_weather"]["temperature"].as_f64()?;
        let weathercode = json["current_weather"]["weathercode"].as_u64().unwrap_or(0) as u8;

        Some(WeatherData { temp, weathercode })
    }

    pub fn get_primary_condition(&self) -> WeatherCondition {
        if !self.enabled {
            return WeatherCondition::Clear;
        }

        if let Ok(guard) = self.cache.lock() {
            guard.cached_condition
        } else {
            WeatherCondition::Clear
        }
    }

    pub fn get_weather_info(&self) -> String {
        if !self.enabled {
            return "Offline Mode".to_string();
        }

        if let Ok(guard) = self.cache.lock() {
            let should_refetch = if let Some(last) = guard.last_fetch {
                last.elapsed() > Duration::from_secs(900)
            } else {
                !guard.is_fetching
            };

            let cached = guard.cached_info.clone();
            drop(guard);

            if should_refetch {
                self.trigger_fetch_background();
            }

            cached
        } else {
            "Concepción · N/A  │  Santiago · N/A".to_string()
        }
    }

    pub fn is_southern_hemisphere(&self) -> bool {
        if let Ok(guard) = self.cache.lock() {
            if let Some(ref g) = guard.gnome_loc {
                return g.lat < 0.0;
            }
        }

        // Check /etc/localtime symlink
        if let Ok(link) = std::fs::read_link("/etc/localtime") {
            let path_str = link.to_string_lossy();
            if path_str.contains("Santiago")
                || path_str.contains("Chile")
                || path_str.contains("Punta_Arenas")
                || path_str.contains("Easter")
                || path_str.contains("Argentina")
                || path_str.contains("America/Sao_Paulo")
                || path_str.contains("Australia")
                || path_str.contains("Auckland")
            {
                return true;
            }
        }

        // Check TZ environment variable
        if let Ok(tz) = std::env::var("TZ") {
            if tz.contains("Santiago") || tz.contains("Chile") || tz.contains("CLT") || tz.contains("CLST") {
                return true;
            }
        }

        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeatherFxKind {
    RainDrop,
    RainSplash,
    SnowFlake,
    FogWisp,
    SunGlimmer,
    CloudWisp,
}

#[derive(Debug, Clone)]
pub struct WeatherParticle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub phase: f32,
    pub life: f32,
    pub max_life: f32,
    pub ch: char,
    pub kind: WeatherFxKind,
}

pub struct WeatherFxEngine {
    pub particles: Vec<WeatherParticle>,
    pub width: u16,
    pub height: u16,
    pub time: f32,
}

impl WeatherFxEngine {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            particles: Vec::with_capacity(128),
            width,
            height,
            time: 0.0,
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        self.particles.retain(|p| p.x < width as f32 && p.y < height as f32);
    }

    pub fn tick(&mut self, condition: WeatherCondition, wind_force: f32) {
        self.time += 0.033;
        let mut rng = rand::thread_rng();
        use rand::Rng;
        let width = self.width;
        let height = self.height;
        let ground_row = height.saturating_sub(2) as f32;

        let target_count = match condition {
            WeatherCondition::Rain => 55,
            WeatherCondition::Snow => 40,
            WeatherCondition::Fog => 22,
            WeatherCondition::Cloudy => 15,
            WeatherCondition::Clear => 16,
        };

        // 1. Update Existing Weather Particles
        for p in &mut self.particles {
            p.life += 0.033;
            p.phase += 0.05;

            match p.kind {
                WeatherFxKind::RainDrop => {
                    p.x += p.vx + (wind_force * 0.75);
                    p.y += p.vy;
                    p.ch = if wind_force > 0.4 {
                        '\\'
                    } else if wind_force < -0.4 {
                        '/'
                    } else {
                        '│'
                    };
                }
                WeatherFxKind::RainSplash => {
                    p.ch = if p.life < 0.08 {
                        'o'
                    } else if p.life < 0.18 {
                        'c'
                    } else {
                        '.'
                    };
                }
                WeatherFxKind::SnowFlake => {
                    let sway = (p.phase + self.time).sin() * 0.35;
                    p.x += p.vx + (wind_force * 0.4) + sway;
                    p.y += p.vy;
                }
                WeatherFxKind::FogWisp | WeatherFxKind::CloudWisp => {
                    p.x += p.vx + (wind_force * 0.2);
                    p.y += p.phase.sin() * 0.04;
                }
                WeatherFxKind::SunGlimmer => {
                    p.x += (p.phase.sin() * 0.12) + (wind_force * 0.1);
                    p.y += p.vy;
                }
            }
        }

        // 2. Ground Collision & Splashes
        let mut new_splashes = Vec::with_capacity(16);
        self.particles.retain(|p| {
            if p.kind == WeatherFxKind::RainDrop && p.y >= ground_row {
                if new_splashes.len() < 16 {
                    new_splashes.push(WeatherParticle {
                        x: p.x.clamp(0.0, width.saturating_sub(1) as f32),
                        y: ground_row,
                        vx: 0.0,
                        vy: 0.0,
                        phase: 0.0,
                        life: 0.0,
                        max_life: 0.26,
                        ch: 'o',
                        kind: WeatherFxKind::RainSplash,
                    });
                }
                false
            } else if p.kind == WeatherFxKind::RainSplash {
                p.life < p.max_life
            } else {
                p.y < ground_row && p.x >= 0.0 && p.x < width as f32 && p.life < p.max_life
            }
        });

        self.particles.extend(new_splashes);

        // 3. Spawn New Condition-Specific Particles
        while self.particles.len() < target_count {
            let p = match condition {
                WeatherCondition::Rain => {
                    let chars = ['│', '┆', '\'', '|'];
                    let ch = chars[rng.gen_range(0..chars.len())];
                    WeatherParticle {
                        x: rng.gen_range(0.0..width.max(1) as f32),
                        y: rng.gen_range(0.0..height.max(1) as f32 / 3.0),
                        vx: rng.gen_range(-0.1..0.1),
                        vy: rng.gen_range(1.2..2.2),
                        phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
                        life: 0.0,
                        max_life: 4.0,
                        ch,
                        kind: WeatherFxKind::RainDrop,
                    }
                }
                WeatherCondition::Snow => {
                    let chars = ['*', '·', '+', 'o'];
                    let ch = chars[rng.gen_range(0..chars.len())];
                    WeatherParticle {
                        x: rng.gen_range(0.0..width.max(1) as f32),
                        y: rng.gen_range(0.0..height.max(1) as f32 / 2.0),
                        vx: rng.gen_range(-0.15..0.15),
                        vy: rng.gen_range(0.22..0.55),
                        phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
                        life: 0.0,
                        max_life: 8.0,
                        ch,
                        kind: WeatherFxKind::SnowFlake,
                    }
                }
                WeatherCondition::Fog => {
                    let chars = ['~', '─', '·', '-'];
                    let ch = chars[rng.gen_range(0..chars.len())];
                    WeatherParticle {
                        x: rng.gen_range(0.0..width.max(1) as f32),
                        y: rng.gen_range(height.max(1) as f32 * 0.35..ground_row),
                        vx: rng.gen_range(0.08..0.28),
                        vy: 0.0,
                        phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
                        life: 0.0,
                        max_life: rng.gen_range(6.0..12.0),
                        ch,
                        kind: WeatherFxKind::FogWisp,
                    }
                }
                WeatherCondition::Cloudy => {
                    let chars = ['~', '─'];
                    let ch = chars[rng.gen_range(0..chars.len())];
                    WeatherParticle {
                        x: rng.gen_range(0.0..width.max(1) as f32),
                        y: rng.gen_range(0.0..height.max(1) as f32 * 0.22),
                        vx: rng.gen_range(0.06..0.18),
                        vy: 0.0,
                        phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
                        life: 0.0,
                        max_life: rng.gen_range(8.0..15.0),
                        ch,
                        kind: WeatherFxKind::CloudWisp,
                    }
                }
                WeatherCondition::Clear => {
                    let chars = ['·', '+', '*'];
                    let ch = chars[rng.gen_range(0..chars.len())];
                    WeatherParticle {
                        x: rng.gen_range(0.0..width.max(1) as f32),
                        y: rng.gen_range(0.0..height.max(1) as f32 * 0.35),
                        vx: rng.gen_range(-0.08..0.08),
                        vy: rng.gen_range(-0.08..0.08),
                        phase: rng.gen_range(0.0..std::f32::consts::PI * 2.0),
                        life: 0.0,
                        max_life: rng.gen_range(4.0..8.0),
                        ch,
                        kind: WeatherFxKind::SunGlimmer,
                    }
                }
            };
            self.particles.push(p);
        }
    }
}

