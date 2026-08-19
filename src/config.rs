use clap::Parser;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mood {
    Day,
    Night,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Season {
    Auto,
    Spring,
    Summer,
    Autumn,
    Winter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hemisphere {
    Auto,
    North,
    South,
}

#[derive(Parser, Debug, Clone)]
#[command(name = "bloom-rust", author = "iapizarro", version = "0.1.0", about = "Nord-themed Terminal Cherry Blossom Screensaver")]
pub struct CliArgs {
    #[arg(short, long, help = "Number of falling blossoms / speed (1-10)")]
    pub speed: Option<f32>,

    #[arg(short = 'w', long, help = "Wind sway magnitude")]
    pub sway: Option<f32>,

    #[arg(short, long, help = "Custom ASCII art file path")]
    pub art: Option<String>,

    #[arg(long, help = "Enable twinkling stars in night mode")]
    pub stars: bool,

    #[arg(short, long, help = "Color mood: day or night")]
    pub mood: Option<String>,

    #[arg(long, help = "Override season: spring, summer, autumn, winter, auto")]
    pub season: Option<String>,

    #[arg(long, help = "Hemisphere: north | south | auto (default: auto)")]
    pub hemisphere: Option<String>,

    #[arg(long, help = "Disable real-time weather integration")]
    pub no_weather: bool,

    #[arg(long, help = "Enable real-time weather integration (default: enabled)")]
    pub weather: bool,

    #[arg(long, help = "City for weather lookup")]
    pub city: Option<String>,

    #[arg(long, help = "Show about page")]
    pub about: bool,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub speed: f32,
    pub sway: f32,
    pub art_path: Option<String>,
    pub stars: bool,
    pub mood: Mood,
    pub season: Season,
    pub hemisphere: Hemisphere,
    pub enable_weather: bool,
    pub city: String,
}

#[allow(dead_code)]
pub enum ConfigActionResult {
    Run(AppConfig),
    Help,
    About,
    Version,
}

pub fn parse_cli_args() -> ConfigActionResult {
    let args = CliArgs::parse();

    if args.about {
        return ConfigActionResult::About;
    }

    let mood = match args.mood.as_deref() {
        Some("day") | Some("Day") => Mood::Day,
        _ => Mood::Night,
    };

    let season = match args.season.as_deref() {
        Some("spring") | Some("Spring") => Season::Spring,
        Some("summer") | Some("Summer") => Season::Summer,
        Some("autumn") | Some("Autumn") | Some("fall") | Some("Fall") => Season::Autumn,
        Some("winter") | Some("Winter") => Season::Winter,
        _ => Season::Auto,
    };

    let hemisphere = match args.hemisphere.as_deref() {
        Some("south") | Some("South") => Hemisphere::South,
        Some("north") | Some("North") => Hemisphere::North,
        _ => Hemisphere::Auto,
    };

    let config = AppConfig {
        speed: args.speed.unwrap_or(1.0).clamp(0.1, 10.0),
        sway: args.sway.unwrap_or(1.0).clamp(0.0, 5.0),
        art_path: args.art,
        stars: args.stars || true,
        mood,
        season,
        hemisphere,
        enable_weather: !args.no_weather,
        city: args.city.unwrap_or_default(),
    };

    ConfigActionResult::Run(config)
}

pub fn print_help() {
    println!("bloom-rust 0.1.0 by iapizarro");
    println!("Nord-themed Terminal Cherry Blossom Screensaver");
    println!("\nUsage: bloom-rust [OPTIONS]\n");
    println!("Options:");
    println!("  -s, --speed <SPEED>       Petal falling speed multiplier (0.1 - 10.0)");
    println!("  -w, --sway <SWAY>         Wind sway amplitude multiplier");
    println!("  -a, --art <FILE>          Custom ASCII art file path");
    println!("      --stars               Enable twinkling starry background (default: on)");
    println!("  -m, --mood <MOOD>         Theme mood: day | night (default: night)");
    println!("      --season <SEASON>     Override season: spring | summer | autumn | winter | auto");
    println!("      --hemisphere <HEMI>   Hemisphere: north | south | auto (default: auto)");
    println!("      --no-weather          Disable real-time weather integration");
    println!("      --city <CITY>         City for weather query");
    println!("      --about               Show about page");
    println!("  -h, --help                Print help");
    println!("  -V, --version             Print version");
}

pub fn print_version() {
    println!("bloom-rust v0.1.0");
}
