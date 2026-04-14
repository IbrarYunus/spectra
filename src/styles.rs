use crate::dsp::Analyzer;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style as RStyle},
};

#[derive(Clone, Copy, Debug)]
pub enum Theme {
    Rainbow,
    Fire,
    Ocean,
    Mono,
    Magma,
}

#[derive(Clone, Copy, Debug)]
pub enum Style {
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

impl Style {
    pub fn all() -> &'static [Style] {
        &[
            Style::Bars,
            Style::Mirror,
            Style::Wave,
            Style::Spectro,
            Style::BarsWave,
            Style::Blocks,
            Style::Radial,
            Style::Lissajous,
            Style::Matrix,
            Style::Particles,
        ]
    }
    pub fn name(&self) -> &'static str {
        match self {
            Style::Bars => "bars",
            Style::Mirror => "mirror",
            Style::Wave => "wave",
            Style::Spectro => "spectro",
            Style::BarsWave => "bars+wave",
            Style::Blocks => "blocks",
            Style::Radial => "radial",
            Style::Lissajous => "lissajous",
            Style::Matrix => "matrix",
            Style::Particles => "particles",
        }
    }
    pub fn next(self) -> Style {
        let all = Self::all();
        let idx = all.iter().position(|s| s.name() == self.name()).unwrap_or(0);
        all[(idx + 1) % all.len()]
    }
    pub fn prev(self) -> Style {
        let all = Self::all();
        let idx = all.iter().position(|s| s.name() == self.name()).unwrap_or(0);
        all[(idx + all.len() - 1) % all.len()]
    }
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (u8, u8, u8) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h6 = h * 6.0;
    let x = c * (1.0 - ((h6 % 2.0) - 1.0).abs());
    let (r, g, b) = match h6 as i32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (
        ((r + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((g + m) * 255.0).clamp(0.0, 255.0) as u8,
        ((b + m) * 255.0).clamp(0.0, 255.0) as u8,
    )
}

fn lerp(a: (u8, u8, u8), b: (u8, u8, u8), t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    (
        (a.0 as f32 + (b.0 as f32 - a.0 as f32) * t) as u8,
        (a.1 as f32 + (b.1 as f32 - a.1 as f32) * t) as u8,
        (a.2 as f32 + (b.2 as f32 - a.2 as f32) * t) as u8,
    )
}

fn gradient(stops: &[(u8, u8, u8)], t: f32) -> (u8, u8, u8) {
    let t = t.clamp(0.0, 1.0);
    if stops.len() < 2 {
        return stops.first().copied().unwrap_or((255, 255, 255));
    }
    let seg = t * (stops.len() - 1) as f32;
    let i = (seg as usize).min(stops.len() - 2);
    let f = seg - i as f32;
    lerp(stops[i], stops[i + 1], f)
}

pub fn theme_color(theme: Theme, t: f32) -> Color {
    let (r, g, b) = match theme {
        Theme::Rainbow => {
            let h = (0.66 - 0.66 * t).rem_euclid(1.0);
            hsl_to_rgb(h, 0.95, 0.55)
        }
        Theme::Fire => gradient(
            &[(20, 0, 0), (120, 10, 0), (220, 60, 0), (255, 160, 0), (255, 240, 180)],
            t,
        ),
        Theme::Ocean => gradient(
            &[(5, 10, 40), (10, 60, 140), (30, 160, 220), (170, 230, 255)],
            t,
        ),
        Theme::Mono => {
            let v = (40.0 + 215.0 * t) as u8;
            (v, v, v)
        }
        Theme::Magma => gradient(
            &[(0, 0, 10), (60, 15, 90), (180, 40, 100), (250, 120, 60), (255, 230, 160)],
            t,
        ),
    };
    Color::Rgb(r, g, b)
}

const EIGHTHS: [char; 9] = [' ', '▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

pub struct MatrixDrop {
    pub col: u16,
    pub y: f32,
    pub speed: f32,
    pub len: usize,
    pub chars: Vec<char>,
}

pub struct Particle {
    pub x: f32,
    pub y: f32,
    pub vx: f32,
    pub vy: f32,
    pub life: f32,
    pub max_life: f32,
    pub hue: f32,
}

pub struct RenderState {
    pub spectro: Vec<Vec<f32>>,
    pub spectro_cols: usize,
    pub drops: Vec<MatrixDrop>,
    pub particles: Vec<Particle>,
    pub rng: u64,
}

impl RenderState {
    pub fn new() -> Self {
        Self {
            spectro: Vec::new(),
            spectro_cols: 0,
            drops: Vec::new(),
            particles: Vec::new(),
            rng: 0x9E3779B97F4A7C15,
        }
    }
}

fn next_u64(state: &mut u64) -> u64 {
    let mut x = *state;
    if x == 0 {
        x = 0x9E3779B97F4A7C15;
    }
    x ^= x << 13;
    x ^= x >> 7;
    x ^= x << 17;
    *state = x;
    x
}

fn rng_f32(state: &mut u64) -> f32 {
    (next_u64(state) >> 40) as f32 / ((1u64 << 24) as f32)
}

// Half-cell buffer: upper/lower halves per terminal cell for 2x vertical resolution.
struct HalfCell {
    area: Rect,
    up: Vec<Option<(u8, u8, u8)>>,
    lo: Vec<Option<(u8, u8, u8)>>,
}

impl HalfCell {
    fn new(area: Rect) -> Self {
        let n = area.width as usize * area.height as usize;
        Self {
            area,
            up: vec![None; n],
            lo: vec![None; n],
        }
    }
    fn plot(&mut self, x: i32, subrow: i32, color: Color) {
        let rgb = match color {
            Color::Rgb(r, g, b) => (r, g, b),
            _ => (255, 255, 255),
        };
        if x < 0 || subrow < 0 {
            return;
        }
        let x = x as u16;
        let subrow = subrow as u16;
        if x >= self.area.width || subrow >= self.area.height * 2 {
            return;
        }
        let row = subrow / 2;
        let idx = (x + row * self.area.width) as usize;
        if subrow % 2 == 0 {
            self.up[idx] = Some(rgb);
        } else {
            self.lo[idx] = Some(rgb);
        }
    }
    fn flush(&self, buf: &mut Buffer) {
        for row in 0..self.area.height {
            for col in 0..self.area.width {
                let idx = (col + row * self.area.width) as usize;
                let u = self.up[idx];
                let l = self.lo[idx];
                let x = self.area.x + col;
                let y = self.area.y + row;
                match (u, l) {
                    (None, None) => {}
                    (Some((r, g, b)), None) => {
                        buf.get_mut(x, y).set_char('▀').set_fg(Color::Rgb(r, g, b));
                    }
                    (None, Some((r, g, b))) => {
                        buf.get_mut(x, y).set_char('▄').set_fg(Color::Rgb(r, g, b));
                    }
                    (Some(uc), Some(lc)) if uc == lc => {
                        buf.get_mut(x, y)
                            .set_char('█')
                            .set_fg(Color::Rgb(uc.0, uc.1, uc.2));
                    }
                    (Some(uc), Some(lc)) => {
                        buf.get_mut(x, y)
                            .set_char('▀')
                            .set_fg(Color::Rgb(uc.0, uc.1, uc.2))
                            .set_bg(Color::Rgb(lc.0, lc.1, lc.2));
                    }
                }
            }
        }
    }
}

// Thai script characters — renders beautifully with the Google font "Pridi".
// See the README for terminal font setup instructions.
const MATRIX_CHARS: &[char] = &[
    'ก', 'ข', 'ค', 'ฆ', 'ง', 'จ', 'ฉ', 'ช', 'ซ', 'ญ', 'ฎ', 'ฏ', 'ฐ', 'ฑ', 'ฒ', 'ณ',
    'ด', 'ต', 'ถ', 'ท', 'ธ', 'น', 'บ', 'ป', 'ผ', 'ฝ', 'พ', 'ฟ', 'ภ', 'ม', 'ย', 'ร',
    'ล', 'ว', 'ศ', 'ษ', 'ส', 'ห', 'ฬ', 'อ', 'ฮ',
    '๐', '๑', '๒', '๓', '๔', '๕', '๖', '๗', '๘', '๙',
];

fn matrix_char(rng: &mut u64) -> char {
    MATRIX_CHARS[(next_u64(rng) as usize) % MATRIX_CHARS.len()]
}

pub fn render(
    style: Style,
    theme: Theme,
    area: Rect,
    buf: &mut Buffer,
    analyzer: &Analyzer,
    state: &mut RenderState,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    match style {
        Style::Bars => render_bars(area, buf, analyzer, theme, false),
        Style::Mirror => render_mirror(area, buf, analyzer, theme),
        Style::Wave => render_wave(area, buf, analyzer, theme),
        Style::Spectro => render_spectro(area, buf, analyzer, theme, state),
        Style::BarsWave => render_bars_wave(area, buf, analyzer, theme),
        Style::Blocks => render_bars(area, buf, analyzer, theme, true),
        Style::Radial => render_radial(area, buf, analyzer, theme),
        Style::Lissajous => render_lissajous(area, buf, analyzer, theme),
        Style::Matrix => render_matrix(area, buf, analyzer, theme, state),
        Style::Particles => render_particles(area, buf, analyzer, theme, state),
    }
}

fn render_radial(area: Rect, buf: &mut Buffer, a: &Analyzer, theme: Theme) {
    let w = area.width as i32;
    let h = area.height as i32;
    if w < 6 || h < 4 {
        return;
    }
    let mut half = HalfCell::new(area);
    let cx = w as f32 / 2.0;
    let cy = (h * 2) as f32 / 2.0; // sub-row space
    // Terminal cells are ~2x tall: in sub-row space vertical resolution is doubled → closer to square.
    let max_r = (w.min(h * 2) as f32 / 2.0 - 1.0).max(3.0);
    let inner = max_r * 0.22;
    let n = a.smooth.len().max(1);

    for (i, &v) in a.smooth.iter().enumerate() {
        let theta = (i as f32 + 0.5) / n as f32 * std::f32::consts::TAU
            - std::f32::consts::FRAC_PI_2;
        let v = v.clamp(0.0, 1.0);
        let outer = inner + v * (max_r - inner);
        let peak = inner + a.peaks[i].clamp(0.0, 1.0) * (max_r - inner);
        let steps = ((outer - inner) * 2.0).ceil() as i32 + 1;
        let (sin, cos) = theta.sin_cos();
        for s in 0..=steps {
            let t = s as f32 / steps.max(1) as f32;
            let r = inner + (outer - inner) * t;
            let x = cx + cos * r;
            let y = cy + sin * r;
            let color = theme_color(theme, (r / max_r).clamp(0.0, 1.0));
            half.plot(x.round() as i32, y.round() as i32, color);
        }
        // peak dot
        let px = cx + cos * peak;
        let py = cy + sin * peak;
        half.plot(
            px.round() as i32,
            py.round() as i32,
            theme_color(theme, 1.0),
        );
    }
    half.flush(buf);
}

fn render_lissajous(area: Rect, buf: &mut Buffer, a: &Analyzer, theme: Theme) {
    let w = area.width as i32;
    let h = area.height as i32;
    if w < 4 || h < 4 {
        return;
    }
    let samples = &a.raw_samples;
    if samples.len() < 64 {
        return;
    }
    let mut half = HalfCell::new(area);
    let cx = w as f32 / 2.0;
    let cy = (h * 2) as f32 / 2.0;
    let scale = (w.min(h * 2) as f32 / 2.0 - 1.0) * 0.9;

    // tail decay
    let delay = 24.min(samples.len() / 4);
    let count = samples.len() - delay;
    for i in 0..count {
        let x_s = samples[i].clamp(-1.2, 1.2);
        let y_s = samples[i + delay].clamp(-1.2, 1.2);
        let x = cx + x_s * scale;
        let y = cy + y_s * scale;
        let t = i as f32 / count as f32;
        let color = theme_color(theme, t);
        half.plot(x.round() as i32, y.round() as i32, color);
    }
    half.flush(buf);
}

fn render_matrix(
    area: Rect,
    buf: &mut Buffer,
    a: &Analyzer,
    theme: Theme,
    state: &mut RenderState,
) {
    let w = area.width;
    let h = area.height;
    if w == 0 || h == 0 {
        return;
    }
    let n = a.smooth.len();
    let bass_end = (n / 4).max(1);
    let bass: f32 = a.smooth[..bass_end].iter().sum::<f32>() / bass_end as f32;
    let overall: f32 = a.smooth.iter().sum::<f32>() / n.max(1) as f32;

    // Drop is done once its last trail char has fallen off the bottom.
    state.drops.retain(|d| d.y - d.len as f32 <= h as f32 + 2.0);

    // One drop per column at a time — no overlapping streams.
    let occupied: std::collections::HashSet<u16> =
        state.drops.iter().map(|d| d.col).collect();

    // Spawn probability scales with bass (kicks trigger new streams) plus a small baseline
    // so the rain never fully stops during quiet passages.
    let spawn_p = (bass * 0.9 + 0.04).min(0.55);
    let free_cols: Vec<u16> =
        (0..w).filter(|c| !occupied.contains(c)).collect();
    let attempts = ((spawn_p * free_cols.len() as f32 * 0.2).ceil() as usize).min(free_cols.len());
    for _ in 0..attempts {
        if free_cols.is_empty() {
            break;
        }
        let idx = (next_u64(&mut state.rng) as usize) % free_cols.len();
        let col = free_cols[idx];
        if rng_f32(&mut state.rng) > spawn_p {
            continue;
        }
        if state.drops.iter().any(|d| d.col == col) {
            continue;
        }
        let len = 6 + (rng_f32(&mut state.rng) * 14.0) as usize;
        let speed = 0.25 + rng_f32(&mut state.rng) * 0.9;
        let chars: Vec<char> = (0..len).map(|_| matrix_char(&mut state.rng)).collect();
        // Randomize starting offset slightly above the top so streams don't all line up.
        let start_y = -(rng_f32(&mut state.rng) * 4.0);
        state.drops.push(MatrixDrop {
            col,
            y: start_y,
            speed,
            len,
            chars,
        });
    }

    let speed_mul = 0.35 + overall * 1.6;

    // CRT phosphor palette: head is bright white-green, next two are bold bright theme,
    // tail fades through theme to near-black. Bold on the top 3 chars gives the glow.
    let head_col = Color::Rgb(220, 255, 220);

    for drop in &mut state.drops {
        drop.y += drop.speed * speed_mul;
        let head_y = drop.y as i32;
        let x = area.x + drop.col;
        if x >= area.x + area.width {
            continue;
        }
        for k in 0..drop.len {
            let y_abs = head_y - k as i32;
            if y_abs < area.y as i32 || y_abs >= (area.y + area.height) as i32 {
                continue;
            }
            let ch = drop.chars[k];
            let (color, bold) = match k {
                0 => (head_col, true),
                1 => (theme_color(theme, 0.95), true),
                2 => (theme_color(theme, 0.78), true),
                _ => {
                    let fade = 1.0 - k as f32 / drop.len as f32;
                    (theme_color(theme, (fade * 0.7).max(0.08)), false)
                }
            };
            let mut st = RStyle::default().fg(color);
            if bold {
                st = st.add_modifier(Modifier::BOLD);
            }
            buf.get_mut(x, y_abs as u16).set_char(ch).set_style(st);
        }
    }
}

fn render_particles(
    area: Rect,
    buf: &mut Buffer,
    a: &Analyzer,
    theme: Theme,
    state: &mut RenderState,
) {
    let w = area.width as f32;
    let h = area.height as f32;
    if w < 2.0 || h < 2.0 {
        return;
    }
    let n = a.smooth.len().max(1);

    for (i, &v) in a.smooth.iter().enumerate() {
        if v > 0.25 && rng_f32(&mut state.rng) < (v - 0.2) * 0.5 {
            let x_norm = (i as f32 + 0.5) / n as f32;
            let x = x_norm * w;
            let y = h - 1.0;
            let vx = (rng_f32(&mut state.rng) - 0.5) * 0.8;
            let vy = -(v * 2.5 + rng_f32(&mut state.rng) * 1.2);
            let life = 0.7 + v * 1.0;
            state.particles.push(Particle {
                x,
                y,
                vx,
                vy,
                life,
                max_life: life,
                hue: x_norm,
            });
        }
    }

    for p in &mut state.particles {
        p.x += p.vx;
        p.y += p.vy;
        p.vy += 0.14;
        p.life -= 0.035;
    }
    state.particles.retain(|p| {
        p.life > 0.0 && p.y < h && p.y > -1.0 && p.x >= 0.0 && p.x < w
    });
    if state.particles.len() > 3000 {
        let n = state.particles.len();
        state.particles.drain(0..(n - 3000));
    }

    for p in &state.particles {
        let xi = p.x as u16;
        let yi = p.y.max(0.0) as u16;
        if xi >= area.width || yi >= area.height {
            continue;
        }
        let t = (p.life / p.max_life).clamp(0.0, 1.0);
        let color = theme_color(theme, (p.hue * 0.7 + 0.3 * t).clamp(0.0, 1.0));
        let ch = if t > 0.8 {
            '●'
        } else if t > 0.5 {
            '•'
        } else if t > 0.25 {
            '·'
        } else {
            '.'
        };
        buf.get_mut(area.x + xi, area.y + yi)
            .set_char(ch)
            .set_fg(color);
    }
}

fn render_bars(area: Rect, buf: &mut Buffer, a: &Analyzer, theme: Theme, blocky: bool) {
    let h = area.height as usize;
    let w = area.width as usize;
    let n = a.smooth.len();
    if h == 0 || n == 0 || w == 0 {
        return;
    }
    let (bar_w, gap) = bar_layout(w, n);
    if bar_w == 0 {
        return;
    }
    let total_w = n * bar_w + n.saturating_sub(1) * gap;
    let start_x = area.x + ((w - total_w.min(w)) / 2) as u16;

    for (i, &v) in a.smooth.iter().enumerate() {
        let bx = start_x + (i * (bar_w + gap)) as u16;
        if bx >= area.x + area.width {
            break;
        }
        let total_eighths = (v.clamp(0.0, 1.0) * (h as f32 * 8.0)) as usize;
        let full = total_eighths / 8;
        let rem = total_eighths % 8;
        let peak_row = ((a.peaks[i].clamp(0.0, 1.0)) * h as f32) as usize;

        for col in 0..bar_w {
            let x = bx + col as u16;
            if x >= area.x + area.width {
                break;
            }
            for row in 0..h {
                let y = area.y + (h - 1 - row) as u16;
                let t = row as f32 / h.max(1) as f32;
                let color = theme_color(theme, t);
                let ch = if row < full {
                    '█'
                } else if row == full && rem > 0 && !blocky {
                    EIGHTHS[rem]
                } else if row == full && blocky && v > 0.05 {
                    '█'
                } else {
                    ' '
                };
                if ch != ' ' {
                    buf.get_mut(x, y).set_char(ch).set_fg(color);
                }
            }
            if peak_row > 0 && peak_row <= h {
                let y = area.y + (h - peak_row) as u16;
                let t = (peak_row as f32 / h as f32).clamp(0.0, 1.0);
                buf.get_mut(x, y)
                    .set_char('▀')
                    .set_fg(theme_color(theme, t));
            }
        }
    }
}

fn bar_layout(width: usize, n: usize) -> (usize, usize) {
    if n == 0 {
        return (0, 0);
    }
    if n >= width {
        return (1, 0);
    }
    // Prefer 1-cell gap between bars when there's room for ≥2-wide bars + gap.
    let with_gap = (width + 1) / (n + 0) / 1; // rough
    let per = width / n;
    if per >= 3 && n > 1 {
        let gap = 1;
        let bar_w = (width - (n - 1) * gap) / n;
        (bar_w.max(1), gap)
    } else {
        let _ = with_gap;
        (per.max(1), 0)
    }
}

fn render_mirror(area: Rect, buf: &mut Buffer, a: &Analyzer, theme: Theme) {
    let h = area.height as usize;
    let w = area.width as usize;
    let n = a.smooth.len();
    if h < 2 || n == 0 || w == 0 {
        return;
    }
    let half = h / 2;
    let (bar_w, gap) = bar_layout(w, n);
    if bar_w == 0 {
        return;
    }
    let total_w = n * bar_w + n.saturating_sub(1) * gap;
    let start_x = area.x + ((w - total_w.min(w)) / 2) as u16;

    for (i, &v) in a.smooth.iter().enumerate() {
        let bx = start_x + (i * (bar_w + gap)) as u16;
        if bx >= area.x + area.width {
            break;
        }
        let total = (v.clamp(0.0, 1.0) * (half as f32 * 8.0)) as usize;
        let full = total / 8;
        let rem = total % 8;
        for col in 0..bar_w {
            let x = bx + col as u16;
            if x >= area.x + area.width {
                break;
            }
            for row in 0..half {
                let t = row as f32 / half.max(1) as f32;
                let color = theme_color(theme, t);
                let y_down = area.y + (half + row) as u16;
                let y_up = area.y + (half - 1 - row) as u16;
                let ch = if row < full {
                    '█'
                } else if row == full && rem > 0 {
                    EIGHTHS[rem]
                } else {
                    ' '
                };
                if ch != ' ' {
                    buf.get_mut(x, y_down).set_char(ch).set_fg(color);
                    let up_ch = if ch == '▁' { '▔' } else { ch };
                    buf.get_mut(x, y_up).set_char(up_ch).set_fg(color);
                }
            }
        }
    }
}

fn render_wave(area: Rect, buf: &mut Buffer, a: &Analyzer, theme: Theme) {
    let w = area.width as usize;
    let h = area.height as usize;
    if w == 0 || h < 2 {
        return;
    }
    let samples = &a.raw_samples;
    if samples.is_empty() {
        return;
    }
    let step = samples.len() as f32 / w as f32;
    let center_f = (h as f32 - 1.0) / 2.0;
    let mut prev_row: Option<usize> = None;
    for col in 0..w {
        let start = (col as f32 * step) as usize;
        let end = ((col + 1) as f32 * step) as usize;
        let end = end.min(samples.len()).max(start + 1);
        let mut peak: f32 = 0.0;
        let mut mean = 0.0;
        for s in &samples[start..end] {
            if s.abs() > peak.abs() {
                peak = *s;
            }
            mean += s;
        }
        mean /= (end - start) as f32;
        let v = peak * 2.2;
        let v = v.clamp(-1.0, 1.0);
        let row = (center_f - v * center_f).round() as isize;
        let row = row.clamp(0, h as isize - 1) as usize;
        let t = (0.5 + mean * 2.0).clamp(0.0, 1.0);
        let color = theme_color(theme, t);
        let x = area.x + col as u16;
        let y = area.y + row as u16;
        buf.get_mut(x, y).set_char('●').set_fg(color);
        if let Some(pr) = prev_row {
            let (lo, hi) = if pr < row { (pr + 1, row) } else { (row + 1, pr) };
            for r in lo..hi {
                let y = area.y + r as u16;
                buf.get_mut(x, y).set_char('│').set_fg(color);
            }
        }
        prev_row = Some(row);
    }
}

fn render_spectro(
    area: Rect,
    buf: &mut Buffer,
    a: &Analyzer,
    theme: Theme,
    state: &mut RenderState,
) {
    let w = area.width as usize;
    let h = area.height as usize;
    if w == 0 || h == 0 {
        return;
    }
    if state.spectro_cols != w {
        state.spectro_cols = w;
        state.spectro.clear();
    }
    state.spectro.push(a.smooth.clone());
    while state.spectro.len() > w {
        state.spectro.remove(0);
    }
    let cols = state.spectro.len();
    for (ci, col) in state.spectro.iter().enumerate() {
        let x = area.x + (w - cols + ci) as u16;
        let n = col.len();
        if n == 0 {
            continue;
        }
        for row in 0..h {
            let bar_idx = ((h - 1 - row) as f32 / h.max(1) as f32 * n as f32) as usize;
            let bar_idx = bar_idx.min(n - 1);
            let v = col[bar_idx].clamp(0.0, 1.0);
            if v < 0.02 {
                continue;
            }
            let color = theme_color(theme, v);
            let y = area.y + row as u16;
            let ch = if v > 0.66 { '█' } else if v > 0.33 { '▓' } else if v > 0.15 { '▒' } else { '░' };
            buf.get_mut(x, y).set_char(ch).set_fg(color);
        }
    }
}

fn render_bars_wave(area: Rect, buf: &mut Buffer, a: &Analyzer, theme: Theme) {
    if area.height < 4 {
        render_bars(area, buf, a, theme, false);
        return;
    }
    let wave_h = (area.height / 3).max(3);
    let bars_area = Rect {
        x: area.x,
        y: area.y,
        width: area.width,
        height: area.height - wave_h,
    };
    let wave_area = Rect {
        x: area.x,
        y: area.y + area.height - wave_h,
        width: area.width,
        height: wave_h,
    };
    render_bars(bars_area, buf, a, theme, false);
    render_wave(wave_area, buf, a, theme);
}
