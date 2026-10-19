//! Procedurally synthesised sound effects (WAV bytes), so the game needs no asset files.

use crate::game::Sfx;

const RATE: u32 = 22050;

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Sine,
    Noise,
    Tri,
}

struct Spec {
    wave: Wave,
    f0: f32,
    f1: f32,
    dur: f32,
    vol: f32,
}

fn spec(s: Sfx) -> Spec {
    use Wave::*;
    let (wave, f0, f1, dur, vol) = match s {
        Sfx::Jump => (Square, 300.0, 600.0, 0.10, 0.18),
        Sfx::Land => (Noise, 200.0, 80.0, 0.06, 0.15),
        Sfx::Jet => (Noise, 900.0, 300.0, 0.25, 0.25),
        Sfx::Splash => (Noise, 1200.0, 200.0, 0.30, 0.25),
        Sfx::Blade => (Tri, 1400.0, 500.0, 0.12, 0.3),
        Sfx::Hit => (Square, 220.0, 90.0, 0.10, 0.25),
        Sfx::Tink => (Square, 1800.0, 1700.0, 0.08, 0.15),
        Sfx::Hurt => (Square, 400.0, 100.0, 0.25, 0.3),
        Sfx::Eat => (Sine, 180.0, 60.0, 0.22, 0.45),
        Sfx::Sonic => (Sine, 2400.0, 600.0, 0.35, 0.2),
        Sfx::Thread => (Tri, 900.0, 1600.0, 0.10, 0.2),
        Sfx::Notice => (Sine, 880.0, 1320.0, 0.14, 0.15),
        Sfx::Break => (Noise, 500.0, 60.0, 0.35, 0.4),
        Sfx::Roar => (Square, 90.0, 50.0, 0.6, 0.3),
        Sfx::Spotted => (Square, 700.0, 1400.0, 0.3, 0.25),
        Sfx::Talk => (Square, 520.0, 520.0, 0.03, 0.06),
        Sfx::Door => (Tri, 120.0, 240.0, 0.5, 0.3),
    };
    Spec { wave, f0, f1, dur, vol }
}

pub const ALL: [Sfx; 17] = [
    Sfx::Jump,
    Sfx::Land,
    Sfx::Jet,
    Sfx::Splash,
    Sfx::Blade,
    Sfx::Hit,
    Sfx::Tink,
    Sfx::Hurt,
    Sfx::Eat,
    Sfx::Sonic,
    Sfx::Thread,
    Sfx::Notice,
    Sfx::Break,
    Sfx::Roar,
    Sfx::Spotted,
    Sfx::Talk,
    Sfx::Door,
];

pub fn wav(s: Sfx) -> Vec<u8> {
    let sp = spec(s);
    let n = (sp.dur * RATE as f32) as usize;
    let mut samples = Vec::with_capacity(n);
    let mut phase = 0.0f32;
    let mut seed = 0x1234_5678u32;
    let mut held = 0.0f32;
    for i in 0..n {
        let t = i as f32 / n as f32;
        let f = sp.f0 + (sp.f1 - sp.f0) * t;
        phase = (phase + f / RATE as f32).fract();
        let v = match sp.wave {
            Wave::Square => if phase < 0.5 { 1.0 } else { -1.0 },
            Wave::Sine => (phase * std::f32::consts::TAU).sin(),
            Wave::Tri => 1.0 - 4.0 * (phase - 0.5).abs(),
            Wave::Noise => {
                // Sample-and-hold noise; the hold rate follows the pitch.
                if phase < f / RATE as f32 {
                    seed ^= seed << 13;
                    seed ^= seed >> 17;
                    seed ^= seed << 5;
                    held = (seed % 2000) as f32 / 1000.0 - 1.0;
                }
                held
            }
        };
        let env = (1.0 - t).powf(1.5) * (t * 60.0).min(1.0);
        samples.push((v * env * sp.vol * 32767.0) as i16);
    }
    to_wav(&samples)
}

fn to_wav(samples: &[i16]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut out = Vec::with_capacity(44 + data_len as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&(36 + data_len).to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&RATE.to_le_bytes());
    out.extend_from_slice(&(RATE * 2).to_le_bytes());
    out.extend_from_slice(&2u16.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for s in samples.iter() {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// A gentle, looping cave ambience: slow minor pads, a soft arpeggio and water drips.
pub fn music() -> Vec<u8> {
    let bpm = 72.0;
    let beat = 60.0 / bpm;
    // A minor, F major, C major, G major (MIDI notes).
    let chords: [[i32; 3]; 4] = [[57, 60, 64], [53, 57, 60], [48, 52, 55], [55, 59, 62]];
    let bars = chords.len();
    let bar_len = beat * 4.0;
    let total = bar_len * bars as f32;
    let n = (total * RATE as f32) as usize;
    let midi = |m: i32| 440.0 * 2f32.powf((m - 69) as f32 / 12.0);
    let mut out = vec![0f32; n];
    let tau = std::f32::consts::TAU;
    for (i, s) in out.iter_mut().enumerate() {
        let t = i as f32 / RATE as f32;
        let bar = ((t / bar_len) as usize).min(bars - 1);
        let tb = t - bar as f32 * bar_len;
        // Pad: crossfade at bar edges so the loop is seamless.
        let fade = (tb / 0.6).min(1.0).min((bar_len - tb) / 0.6).max(0.0);
        let mut v = 0.0;
        for &m in &chords[bar] {
            let f = midi(m - 12);
            v += (t * f * tau).sin() * 0.10 + (t * f * 2.003 * tau).sin() * 0.03;
        }
        v += (t * midi(chords[bar][0] - 24) * tau).sin() * 0.12;
        v *= fade * (0.8 + 0.2 * (t * 0.5 * tau).sin());
        // Arpeggio on eighth notes.
        let step = (tb / (beat / 2.0)) as usize;
        let ts = tb - step as f32 * beat / 2.0;
        let pattern = [0, 1, 2, 1, 0, 2, 1, 2];
        let note = chords[bar][pattern[step % 8]] + 12;
        let ph = (t * midi(note)).fract();
        let tri = 1.0 - 4.0 * (ph - 0.5).abs();
        v += tri * (-ts * 6.0).exp() * 0.07;
        *s = v;
    }
    // Water drips: short high pings at fixed pseudo-random times.
    let mut seed = 0x9e3779b9u32;
    for _ in 0..10 {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let start = (seed as usize) % (n - RATE as usize / 2);
        let f = 1400.0 + (seed % 900) as f32;
        for k in 0..(RATE as usize / 4) {
            let tt = k as f32 / RATE as f32;
            let fr = f * (1.0 + tt * 3.0);
            out[start + k] += (tt * fr * tau).sin() * (-tt * 25.0).exp() * 0.12;
        }
    }
    let samples: Vec<i16> = out.iter().map(|v| (v.clamp(-1.0, 1.0) * 32767.0 * 0.8) as i16).collect();
    to_wav(&samples)
}

