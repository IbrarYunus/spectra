use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::Arc;

pub struct Analyzer {
    fft: Arc<dyn rustfft::Fft<f32>>,
    size: usize,
    window: Vec<f32>,
    sample_rate: u32,
    n_bars: usize,
    pub smooth: Vec<f32>,
    pub peaks: Vec<f32>,
    pub raw_samples: Vec<f32>,
    pub speed: f32,
}

impl Analyzer {
    pub fn new(size: usize, sample_rate: u32, n_bars: usize) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(size);
        let window: Vec<f32> = (0..size)
            .map(|i| {
                0.5 - 0.5
                    * (2.0 * std::f32::consts::PI * i as f32 / (size - 1) as f32).cos()
            })
            .collect();
        Self {
            fft,
            size,
            window,
            sample_rate,
            n_bars,
            smooth: vec![0.0; n_bars],
            peaks: vec![0.0; n_bars],
            raw_samples: vec![0.0; size],
            speed: 1.0,
        }
    }

    pub fn set_bars(&mut self, n: usize) {
        if n == self.n_bars {
            return;
        }
        self.n_bars = n;
        self.smooth = vec![0.0; n];
        self.peaks = vec![0.0; n];
    }

    pub fn window_size(&self) -> usize {
        self.size
    }

    pub fn analyze(&mut self, samples: &[f32]) {
        self.raw_samples.clear();
        self.raw_samples.extend_from_slice(samples);

        let mut buf: Vec<Complex<f32>> = samples
            .iter()
            .zip(&self.window)
            .map(|(s, w)| Complex { re: s * w, im: 0.0 })
            .collect();
        if buf.len() < self.size {
            buf.resize(self.size, Complex { re: 0.0, im: 0.0 });
        }
        self.fft.process(&mut buf);

        let nyquist = self.sample_rate as f32 / 2.0;
        let bin_hz = nyquist / (self.size as f32 / 2.0);
        let fmin: f32 = 30.0;
        let fmax: f32 = 16000.0_f32.min(nyquist - bin_hz);
        let log_min = fmin.ln();
        let log_max = fmax.ln();

        let mut out = vec![0.0; self.n_bars];
        for i in 0..self.n_bars {
            let f0 = (log_min + (log_max - log_min) * i as f32 / self.n_bars as f32).exp();
            let f1 = (log_min
                + (log_max - log_min) * (i + 1) as f32 / self.n_bars as f32)
                .exp();
            let b0 = ((f0 / bin_hz) as usize).max(1);
            let b1 = ((f1 / bin_hz) as usize)
                .max(b0 + 1)
                .min(self.size / 2);
            let mut sum = 0.0;
            for b in b0..b1 {
                let c = buf[b];
                sum += (c.re * c.re + c.im * c.im).sqrt();
            }
            let mag = sum / (b1 - b0) as f32;
            let db = 20.0 * (mag + 1e-9).log10();
            let norm = ((db + 55.0) / 55.0).clamp(0.0, 1.0);
            let tilt = 1.0 + 0.25 * (i as f32 / self.n_bars.max(1) as f32);
            out[i] = (norm * tilt).clamp(0.0, 1.1);
        }

        let s = self.speed.clamp(0.05, 4.0);
        let attack_w = (0.75 * s).clamp(0.05, 0.98);
        let release_w = (0.18 * s).clamp(0.01, 0.9);
        let peak_decay = 0.012 * s;
        for i in 0..self.n_bars {
            let target = out[i];
            if target > self.smooth[i] {
                self.smooth[i] = self.smooth[i] * (1.0 - attack_w) + target * attack_w;
            } else {
                self.smooth[i] = self.smooth[i] * (1.0 - release_w) + target * release_w;
            }
            if self.smooth[i] > self.peaks[i] {
                self.peaks[i] = self.smooth[i];
            } else {
                self.peaks[i] = (self.peaks[i] - peak_decay).max(self.smooth[i]);
            }
        }
    }
}
