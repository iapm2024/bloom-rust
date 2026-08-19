mod art;
mod config;
mod physics;
mod stars;
mod ui;
mod weather;

use anyhow::Result;
use config::{parse_cli_args, ConfigActionResult};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use physics::ParticleEngine;
use ratatui::{backend::CrosstermBackend, Terminal};
use stars::StarrySky;
use std::fs;
use std::io::stdout;
use std::time::Duration;
use ui::render_ui;
use weather::{WeatherFetcher, WeatherFxEngine};

struct TerminalCleanup;

impl Drop for TerminalCleanup {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, crossterm::cursor::Show);
    }
}

fn main() -> Result<()> {
    let mut config = match parse_cli_args() {
        ConfigActionResult::Run(c) => c,
        ConfigActionResult::Help => {
            config::print_help();
            return Ok(());
        }
        ConfigActionResult::About => {
            println!("bloom-rust v0.1 by iapizarro");
            println!("Nord-themed Terminal Cherry Blossom Screensaver written in Rust.");
            return Ok(());
        }
        ConfigActionResult::Version => {
            config::print_version();
            return Ok(());
        }
    };

    let art_data = if let Some(ref path) = config.art_path {
        fs::read_to_string(path).unwrap_or_else(|_| art::DEFAULT_ART_DATA.to_string())
    } else {
        art::DEFAULT_ART_DATA.to_string()
    };

    let tree_grid = art::parse_art(&art_data);

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let _cleanup = TerminalCleanup;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let size = terminal.size()?;
    let weather = WeatherFetcher::new(config.enable_weather, config.city.clone());
    let mut particles = ParticleEngine::new(&tree_grid, size.width, size.height, config.speed, config.sway);
    let mut stars = StarrySky::new(size.width, size.height);
    let mut weather_fx = WeatherFxEngine::new(size.width, size.height);

    let mut show_about = false;
    let target_frame_duration = Duration::from_millis(33); // ~30 FPS
    let mut last_frame = std::time::Instant::now();

    loop {
        terminal.draw(|f| {
            render_ui(f, &config, &weather, &tree_grid, &particles, &stars, &weather_fx, show_about);
        })?;

        let elapsed = last_frame.elapsed();
        let poll_timeout = target_frame_duration.saturating_sub(elapsed);

        if event::poll(poll_timeout)? {
            match event::read()? {
                Event::Key(key) => {
                    if key.kind == KeyEventKind::Press {
                        match key.code {
                            KeyCode::Char('q') | KeyCode::Char('Q') | KeyCode::Esc => {
                                break;
                            }
                            KeyCode::Char('a') | KeyCode::Char('A') | KeyCode::Char('?') => {
                                show_about = !show_about;
                            }
                            KeyCode::Char('1') => {
                                config.season = config::Season::Spring;
                            }
                            KeyCode::Char('2') => {
                                config.season = config::Season::Summer;
                            }
                            KeyCode::Char('3') => {
                                config.season = config::Season::Autumn;
                            }
                            KeyCode::Char('4') => {
                                config.season = config::Season::Winter;
                            }
                            KeyCode::Char('m') | KeyCode::Char('M') => {
                                config.mood = match config.mood {
                                    config::Mood::Day => config::Mood::Night,
                                    config::Mood::Night => config::Mood::Day,
                                };
                            }
                            _ => {}
                        }
                    }
                }
                Event::Resize(w, h) => {
                    particles.resize(w, h);
                    stars.resize(w, h);
                    weather_fx.resize(w, h);
                }
                _ => {}
            }
        }

        let total_elapsed = last_frame.elapsed();
        if total_elapsed < target_frame_duration {
            std::thread::sleep(target_frame_duration - total_elapsed);
        }
        last_frame = std::time::Instant::now();

        let current_condition = weather.get_primary_condition();
        particles.tick(&tree_grid);
        stars.tick();
        weather_fx.tick(current_condition, particles.current_wind);
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(())
}

