mod audio;
mod dsp;
#[cfg(target_os = "macos")]
mod screencap;
mod styles;

use anyhow::Result;
use clap::{Parser, ValueEnum};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders},
    Terminal,
};
use std::{
    io::stdout,
    time::{Duration, Instant},
};

use styles::{render, RenderState, Style, Theme};

#[derive(Clone, Copy, ValueEnum)]
enum StyleArg {
    Bars,
    Mirror,
    Wave,
    Spectro,
    BarsWave,
    Blocks,
    Radial,
    Lissajous,
    Matrix,
    Particles,
}

impl From<StyleArg> for Style {
    fn from(s: StyleArg) -> Self {
        match s {
            StyleArg::Bars => Style::Bars,
            StyleArg::Mirror => Style::Mirror,
            StyleArg::Wave => Style::Wave,
            StyleArg::Spectro => Style::Spectro,
            StyleArg::BarsWave => Style::BarsWave,
            StyleArg::Blocks => Style::Blocks,
            StyleArg::Radial => Style::Radial,
            StyleArg::Lissajous => Style::Lissajous,
            StyleArg::Matrix => Style::Matrix,
            StyleArg::Particles => Style::Particles,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum ThemeArg {
    Rainbow,
    Fire,
    Ocean,
    Mono,
    Magma,
}

impl From<ThemeArg> for Theme {
    fn from(t: ThemeArg) -> Self {
        match t {
            ThemeArg::Rainbow => Theme::Rainbow,
            ThemeArg::Fire => Theme::Fire,
            ThemeArg::Ocean => Theme::Ocean,
            ThemeArg::Mono => Theme::Mono,
            ThemeArg::Magma => Theme::Magma,
        }
    }
}

#[derive(Parser)]
#[command(
    name = "spectra",
    version,
    about = "Terminal music visualizer for macOS — by Ibrar Yunus",
    long_about = "spectra — a fast terminal music visualizer with ten styles, system-audio capture via ScreenCaptureKit, and Thai-script matrix rain.\n\nCrafted by Ibrar Yunus (github.com/IbrarYunus)."
)]
struct Cli {
    /// Audio file to play & visualize (mp3/wav/flac/ogg/m4a)
    #[arg(short, long)]
    file: Option<String>,

    /// Input device name (microphone); omit for default
    #[arg(short, long)]
    device: Option<String>,

    /// Initial visual style
    #[arg(short, long, value_enum, default_value = "bars")]
    style: StyleArg,

    /// Color theme
    #[arg(short, long, value_enum, default_value = "rainbow")]
    theme: ThemeArg,

    /// Frames per second (1-120)
    #[arg(long, default_value_t = 60)]
    fps: u32,

    /// Number of spectrum bars (0 = fit terminal width)
    #[arg(short = 'n', long, default_value_t = 10)]
    bars: usize,

    /// Animation speed: 1.0 = normal, <1 slower (smoother), >1 snappier
    #[arg(long, default_value_t = 0.4)]
    speed: f32,

    /// List available input devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Show author & credits, then exit
    #[arg(long)]
    credits: bool,

    /// Capture system audio via ScreenCaptureKit (macOS 13+).
    /// Requires Screen Recording permission for your terminal app.
    #[cfg(target_os = "macos")]
    #[arg(long)]
    system: bool,

    /// Hide the help/status bar
    #[arg(long)]
    no_ui: bool,
}

fn print_credits() {
    let version = env!("CARGO_PKG_VERSION");
    println!();
    println!("  \x1b[1;36mspectra\x1b[0m v{version}");
    println!("  terminal music visualizer for macOS");
    println!();
    println!("  \x1b[1mAuthor\x1b[0m");
    println!("    Ibrar Yunus — Full-Stack AI Engineer & Data Scientist");
    println!("    University of St Andrews, School of Computer Science");
    println!("    Gold Medal, Final Year Project (Computer Vision for Driving Safety)");
    println!("    OxfordHack 2017 participant");
    println!();
    println!("  \x1b[1mLinks\x1b[0m");
    println!("    Website   https://ibraryunus.com");
    println!("    GitHub    https://github.com/IbrarYunus");
    println!("    LinkedIn  https://www.linkedin.com/in/ibrar-yunus/");
    println!("    X         https://x.com/ibraryunus");
    println!();
    println!("  \x1b[1mThanks\x1b[0m");
    println!("    Built with Rust · cpal · rodio · rustfft · ratatui · crossterm");
    println!("    System audio via Apple ScreenCaptureKit");
    println!("    Matrix rain glyphs rendered best with the Google font \"Pridi\"");
    println!();
}

fn list_input_devices() -> Result<()> {
    use cpal::traits::{DeviceTrait, HostTrait};
    let host = cpal::default_host();
    let default = host.default_input_device().and_then(|d| d.name().ok());
    println!("Input devices:");
    for d in host.input_devices()? {
        let name = d.name().unwrap_or_else(|_| "<unknown>".into());
        let mark = if Some(&name) == default.as_ref() { " (default)" } else { "" };
        println!("  - {name}{mark}");
    }
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.list_devices {
        return list_input_devices();
    }

    if cli.credits {
        print_credits();
        return Ok(());
    }

    let source = {
        #[cfg(target_os = "macos")]
        {
            if cli.system {
                screencap::start_screen_capture()?
            } else if let Some(path) = &cli.file {
                audio::start_file(path)?
            } else {
                audio::start_microphone(cli.device.as_deref())?
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            if let Some(path) = &cli.file {
                audio::start_file(path)?
            } else {
                audio::start_microphone(cli.device.as_deref())?
            }
        }
    };

    let fps = cli.fps.clamp(1, 120);
    let frame_dt = Duration::from_millis((1000 / fps) as u64);

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, crossterm::cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(
        &mut terminal,
        source,
        cli.style.into(),
        cli.theme.into(),
        frame_dt,
        !cli.no_ui,
        cli.bars,
        cli.speed,
    );

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;
    terminal.show_cursor().ok();

    result
}

fn run_app<B: Backend>(
    terminal: &mut Terminal<B>,
    source: audio::AudioSource,
    start_style: Style,
    start_theme: Theme,
    frame_dt: Duration,
    show_ui: bool,
    user_bars: usize,
    start_speed: f32,
) -> Result<()> {
    let window_size = 2048;
    let mut analyzer = dsp::Analyzer::new(window_size, source.sample_rate, user_bars.max(1));
    analyzer.speed = start_speed.clamp(0.05, 4.0);
    let mut bars_override = user_bars;
    let mut speed = analyzer.speed;
    let mut style = start_style;
    let mut theme = start_theme;
    let mut state = RenderState::new();
    let themes = [Theme::Rainbow, Theme::Fire, Theme::Ocean, Theme::Magma, Theme::Mono];
    let mut theme_idx = themes
        .iter()
        .position(|t| std::mem::discriminant(t) == std::mem::discriminant(&theme))
        .unwrap_or(0);

    let mut last_frame = Instant::now();
    let mut fps_counter: f32 = 0.0;
    let mut fps_last = Instant::now();
    let mut fps_frames: u32 = 0;

    loop {
        let timeout = frame_dt
            .checked_sub(last_frame.elapsed())
            .unwrap_or(Duration::ZERO);
        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => break,
                    KeyCode::Char(' ') | KeyCode::Right | KeyCode::Tab => {
                        style = style.next();
                    }
                    KeyCode::Left | KeyCode::BackTab => {
                        style = style.prev();
                    }
                    KeyCode::Char('t') => {
                        theme_idx = (theme_idx + 1) % themes.len();
                        theme = themes[theme_idx];
                    }
                    KeyCode::Char('1') => style = Style::Bars,
                    KeyCode::Char('2') => style = Style::Mirror,
                    KeyCode::Char('3') => style = Style::Wave,
                    KeyCode::Char('4') => style = Style::Spectro,
                    KeyCode::Char('5') => style = Style::BarsWave,
                    KeyCode::Char('6') => style = Style::Blocks,
                    KeyCode::Char('7') => style = Style::Radial,
                    KeyCode::Char('8') => style = Style::Lissajous,
                    KeyCode::Char('9') => style = Style::Matrix,
                    KeyCode::Char('p') => style = Style::Particles,
                    KeyCode::Char('+') | KeyCode::Char('=') => {
                        speed = (speed * 1.25).min(4.0);
                        analyzer.speed = speed;
                    }
                    KeyCode::Char('-') | KeyCode::Char('_') => {
                        speed = (speed / 1.25).max(0.05);
                        analyzer.speed = speed;
                    }
                    KeyCode::Char(']') => bars_override = (bars_override.max(1) + 2).min(512),
                    KeyCode::Char('[') => {
                        bars_override = bars_override.saturating_sub(2).max(2);
                    }
                    KeyCode::Char('0') => bars_override = 0, // auto-fit
                    _ => {}
                }
            }
        }

        if last_frame.elapsed() < frame_dt {
            continue;
        }
        last_frame = Instant::now();

        fps_frames += 1;
        if fps_last.elapsed() >= Duration::from_millis(500) {
            fps_counter = fps_frames as f32 / fps_last.elapsed().as_secs_f32();
            fps_frames = 0;
            fps_last = Instant::now();
        }

        let samples = source.buffer.snapshot(window_size);

        terminal.draw(|f| {
            let size = f.area();
            let (vis_area, status_area) = if show_ui && size.height > 2 {
                (
                    Rect { x: 0, y: 0, width: size.width, height: size.height - 1 },
                    Some(Rect { x: 0, y: size.height - 1, width: size.width, height: 1 }),
                )
            } else {
                (size, None)
            };

            let target_bars = match style {
                Style::Wave | Style::Lissajous => 64,
                Style::Spectro => (vis_area.height as usize).max(16),
                Style::Matrix | Style::Particles => 32,
                Style::Radial => bars_override.max(24).min(96),
                _ => {
                    if bars_override == 0 {
                        vis_area.width as usize
                    } else {
                        bars_override.min(vis_area.width as usize).max(1)
                    }
                }
            };
            analyzer.set_bars(target_bars.max(1));
            analyzer.analyze(&samples);

            let buf = f.buffer_mut();
            render(style, theme, vis_area, buf, &analyzer, &mut state);

            if let Some(status) = status_area {
                let bars_label = if bars_override == 0 {
                    "auto".to_string()
                } else {
                    bars_override.to_string()
                };
                let line = format!(
                    " spectra · by Ibrar Yunus │ {} │ bars: {:<4} speed: {:>4.2} fps: {:>3.0} │ {} │ [space/←→] style  [t] theme  [+/-] speed  [ [ / ] ] bars  [q] quit ",
                    style.name(),
                    bars_label,
                    speed,
                    fps_counter,
                    source.source_label,
                );
                let mut x = status.x;
                for (i, ch) in line.chars().enumerate() {
                    if i as u16 >= status.width {
                        break;
                    }
                    buf.get_mut(x, status.y)
                        .set_char(ch)
                        .set_fg(Color::Black)
                        .set_bg(Color::Rgb(180, 180, 180));
                    x += 1;
                }
                while x < status.x + status.width {
                    buf.get_mut(x, status.y)
                        .set_char(' ')
                        .set_bg(Color::Rgb(180, 180, 180));
                    x += 1;
                }
                let _ = Block::default().borders(Borders::NONE);
            }
        })?;
    }

    Ok(())
}

fn theme_name(t: Theme) -> &'static str {
    match t {
        Theme::Rainbow => "rainbow",
        Theme::Fire => "fire",
        Theme::Ocean => "ocean",
        Theme::Mono => "mono",
        Theme::Magma => "magma",
    }
}
