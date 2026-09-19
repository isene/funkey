//! Sound. Samples are mixed here and piped to one audio child: pw-play,
//! paplay or aplay, whichever is installed. A sample comes from a WAV
//! file, a Doom lump, or is made on the spot: a tone, a slide, noise, or
//! a tune written as notes. A recorder mode plays nothing and writes a
//! WAV of everything a scripted run asked for, for films and tests.

use std::io::Write;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Samples a second, mono, 16 bit.
pub const RATE: u32 = 22050;
const CHUNK: usize = 512;
const LEAD: usize = 2048;
const VOICES: usize = 12;

#[derive(Clone, Debug)]
pub struct Sample {
    pub data: Arc<Vec<i16>>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Wave { Square, Triangle, Saw, Sine }

impl Sample {
    pub fn from_i16(data: Vec<i16>) -> Sample { Sample { data: Arc::new(data) } }

    pub fn secs(&self) -> f32 { self.data.len() as f32 / RATE as f32 }

    /// A WAV file: PCM, 8 or 16 bits, mono or stereo, any rate.
    pub fn from_wav(b: &[u8]) -> Option<Sample> {
        if b.len() < 44 || &b[0..4] != b"RIFF" || &b[8..12] != b"WAVE" { return None; }
        let (mut at, mut rate, mut channels, mut bits) = (12usize, 0u32, 1u16, 16u16);
        let mut pcm: Option<&[u8]> = None;
        while at + 8 <= b.len() {
            let id = &b[at..at + 4];
            let size = u32::from_le_bytes([b[at + 4], b[at + 5], b[at + 6], b[at + 7]]) as usize;
            let body = &b[at + 8..(at + 8 + size).min(b.len())];
            if id == b"fmt " && body.len() >= 16 {
                channels = u16::from_le_bytes([body[2], body[3]]);
                rate = u32::from_le_bytes([body[4], body[5], body[6], body[7]]);
                bits = u16::from_le_bytes([body[14], body[15]]);
            } else if id == b"data" {
                pcm = Some(body);
            }
            at += 8 + size + (size & 1);
        }
        let pcm = pcm?;
        if rate == 0 || channels == 0 { return None; }
        let ch = channels as usize;
        let frames: Vec<i16> = match bits {
            8 => pcm.chunks(ch).map(|f| f.iter().map(|&s| (s as i16 - 128) << 8).sum::<i16>() / ch as i16).collect(),
            16 => pcm.chunks(2 * ch).filter(|f| f.len() == 2 * ch).map(|f| {
                let mut acc = 0i32;
                for c in 0..ch { acc += i16::from_le_bytes([f[c * 2], f[c * 2 + 1]]) as i32; }
                (acc / ch as i32) as i16
            }).collect(),
            _ => return None,
        };
        Some(Sample::from_i16(resample(&frames, rate)))
    }

    /// Doom's sound lump: a small header, then unsigned 8-bit samples.
    pub fn from_doom(b: &[u8]) -> Option<Sample> {
        if b.len() < 8 { return None; }
        let rate = u16::from_le_bytes([b[2], b[3]]) as u32;
        let count = u32::from_le_bytes([b[4], b[5], b[6], b[7]]) as usize;
        let body = &b[8..];
        let n = count.min(body.len());
        let samples = if n > 48 { &body[16..n - 16] } else { &body[..n] };
        let frames: Vec<i16> = samples.iter().map(|&s| (s as i16 - 128) << 8).collect();
        Some(Sample::from_i16(resample(&frames, rate.max(1))))
    }

    /// A steady note with a short fade in and out.
    pub fn tone(wave: Wave, hz: f32, secs: f32, vol: f32) -> Sample { Sample::sweep(wave, hz, hz, secs, vol) }

    /// A steady note with no fade, cut at a whole number of cycles, so it
    /// loops without a click: an engine, a hum.
    pub fn loop_tone(wave: Wave, hz: f32, secs: f32, vol: f32) -> Sample {
        let cycles = (secs * hz).round().max(1.0);
        let n = (cycles * RATE as f32 / hz) as usize;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            let phase = (i as f32 * hz / RATE as f32).fract();
            let v = match wave {
                Wave::Square => if phase < 0.5 { 1.0 } else { -1.0 },
                Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
                Wave::Saw => 2.0 * phase - 1.0,
                Wave::Sine => (phase * std::f32::consts::TAU).sin(),
            };
            out.push((v * vol * 20000.0) as i16);
        }
        Sample::from_i16(out)
    }

    /// A note sliding from one pitch to another: a jump, a fall, a laser.
    pub fn sweep(wave: Wave, from_hz: f32, to_hz: f32, secs: f32, vol: f32) -> Sample {
        let n = (secs * RATE as f32) as usize;
        let mut out = Vec::with_capacity(n);
        let mut phase = 0.0f32;
        for i in 0..n {
            let t = i as f32 / n.max(1) as f32;
            let hz = from_hz + (to_hz - from_hz) * t;
            phase = (phase + hz / RATE as f32).fract();
            let v = match wave {
                Wave::Square => if phase < 0.5 { 1.0 } else { -1.0 },
                Wave::Triangle => 4.0 * (phase - 0.5).abs() - 1.0,
                Wave::Saw => 2.0 * phase - 1.0,
                Wave::Sine => (phase * std::f32::consts::TAU).sin(),
            };
            out.push((v * vol * envelope(i, n) * 20000.0) as i16);
        }
        Sample::from_i16(out)
    }

    /// A burst of noise that dies away: a hit, an explosion.
    pub fn noise(secs: f32, vol: f32) -> Sample {
        let n = (secs * RATE as f32) as usize;
        let mut x = 0x2545_f491u32;
        let mut out = Vec::with_capacity(n);
        for i in 0..n {
            x ^= x << 13; x ^= x >> 17; x ^= x << 5;
            let v = ((x >> 16) as f32 / 32768.0) - 1.0;
            let decay = 1.0 - i as f32 / n.max(1) as f32;
            out.push((v * vol * decay * decay * 20000.0) as i16);
        }
        Sample::from_i16(out)
    }

    /// Two samples one after the other.
    pub fn then(&self, next: &Sample) -> Sample {
        let mut d = (*self.data).clone();
        d.extend_from_slice(&next.data);
        Sample::from_i16(d)
    }
}

/// A short rise and fall so a note does not click.
fn envelope(i: usize, n: usize) -> f32 {
    let edge = (RATE as usize / 200).min(n / 4).max(1);
    if i < edge { i as f32 / edge as f32 } else if n - i < edge { (n - i) as f32 / edge as f32 } else { 1.0 }
}

/// Nearest-sample resampling to RATE.
fn resample(src: &[i16], from: u32) -> Vec<i16> {
    if from == RATE { return src.to_vec(); }
    let n = (src.len() as u64 * RATE as u64 / from as u64) as usize;
    (0..n).map(|i| src[(i as u64 * from as u64 / RATE as u64) as usize]).collect()
}

/// A tune as text: `"120 c4 e4 g4 c5/2 - e4/8"`. The first number is
/// beats a minute. A note is a letter, an optional `#` or `b`, and an
/// octave; `-` is a rest. A quarter note unless `/n` says a whole (1), a
/// half (2), an eighth (8) or a sixteenth (16).
#[derive(Clone, Debug)]
pub struct Tune {
    /// Pitch in Hz (0 for a rest) and length in seconds.
    pub notes: Vec<(f32, f32)>,
    pub wave: Wave,
    pub vol: f32,
}

impl Tune {
    pub fn parse(text: &str, wave: Wave, vol: f32) -> Tune {
        let mut bpm = 120.0f32;
        let mut notes = Vec::new();
        for tok in text.split_whitespace() {
            if let Ok(b) = tok.parse::<f32>() { bpm = b; continue; }
            let (name, len) = tok.split_once('/').unwrap_or((tok, "4"));
            let beats = 4.0 / len.parse::<f32>().unwrap_or(4.0);
            let secs = beats * 60.0 / bpm;
            notes.push((note_hz(name), secs));
        }
        Tune { notes, wave, vol }
    }

    /// The whole tune as one sample.
    pub fn render(&self) -> Sample {
        let mut out: Vec<i16> = Vec::new();
        for &(hz, secs) in &self.notes {
            if hz > 0.0 {
                out.extend_from_slice(&Sample::tone(self.wave, hz, secs * 0.92, self.vol).data);
                out.resize(out.len() + ((secs * 0.08) * RATE as f32) as usize, 0);
            } else {
                out.resize(out.len() + (secs * RATE as f32) as usize, 0);
            }
        }
        Sample::from_i16(out)
    }
}

/// The pitch of a note name such as `c4`, `f#3` or `bb5`; 0 for a rest.
pub fn note_hz(name: &str) -> f32 {
    let b = name.as_bytes();
    if b.is_empty() || b[0] == b'-' { return 0.0; }
    let base = match b[0].to_ascii_lowercase() { b'c' => 0, b'd' => 2, b'e' => 4, b'f' => 5, b'g' => 7, b'a' => 9, b'b' => 11, _ => return 0.0 };
    let mut i = 1;
    let mut semi = base;
    if i < b.len() && b[i] == b'#' { semi += 1; i += 1; } else if i < b.len() && b[i] == b'b' { semi -= 1; i += 1; }
    let octave = if i < b.len() { (b[i] as i32 - b'0' as i32).clamp(0, 9) } else { 4 };
    let n = octave * 12 + semi - 57;
    440.0 * 2f32.powf(n as f32 / 12.0)
}

enum Cmd {
    Play { s: Sample, vol: f32, looped: bool, channel: u8 },
    Stop(u8),
    Volume(u8, f32),
    Rate(u8, f32),
}

enum Mode {
    Off,
    Live { tx: Sender<Cmd>, child: Child },
    Record { t: f32, events: Vec<(f32, Sample, f32)> },
}

pub struct Audio { mode: Mode }

impl Audio {
    /// Open the mixer. Silent when no player is found, or when
    /// `FUNKEY_SOUND=0`.
    pub fn open() -> Audio {
        if std::env::var("FUNKEY_SOUND").map(|v| v == "0").unwrap_or(false) { return Audio::off(); }
        match start_player() { Some((tx, child)) => Audio { mode: Mode::Live { tx, child } }, None => Audio::off() }
    }

    pub fn off() -> Audio { Audio { mode: Mode::Off } }

    /// Plays nothing, remembers everything with its time: see `write_wav`.
    pub fn recorder() -> Audio { Audio { mode: Mode::Record { t: 0.0, events: Vec::new() } } }

    pub fn on(&self) -> bool { !matches!(self.mode, Mode::Off) }

    /// Play a sample once, on any free channel.
    pub fn play(&mut self, s: &Sample, vol: f32) { self.send(Cmd::Play { s: s.clone(), vol, looped: false, channel: 0 }, s, vol); }

    /// Play on a numbered channel (1 to 8), replacing what was there.
    pub fn play_on(&mut self, channel: u8, s: &Sample, vol: f32) { self.send(Cmd::Play { s: s.clone(), vol, looped: false, channel }, s, vol); }

    /// Loop a sample on a numbered channel until `stop`: music, an engine.
    pub fn play_loop(&mut self, channel: u8, s: &Sample, vol: f32) { self.send(Cmd::Play { s: s.clone(), vol, looped: true, channel }, s, vol); }

    pub fn stop(&mut self, channel: u8) { self.send(Cmd::Stop(channel), &Sample::from_i16(Vec::new()), 0.0); }

    /// Change a channel's volume while it plays.
    pub fn volume(&mut self, channel: u8, vol: f32) { if let Mode::Live { tx, .. } = &self.mode { let _ = tx.send(Cmd::Volume(channel, vol)); } }

    /// Change a channel's playback speed: 1.0 as recorded, 2.0 an octave up.
    pub fn rate(&mut self, channel: u8, rate: f32) { if let Mode::Live { tx, .. } = &self.mode { let _ = tx.send(Cmd::Rate(channel, rate.max(0.05))); } }

    /// Recorder: the clock, in seconds, set by the game every tick.
    pub fn set_time(&mut self, secs: f32) { if let Mode::Record { t, .. } = &mut self.mode { *t = secs; } }

    /// Recorder: everything played so far, mixed into a WAV file.
    pub fn write_wav(&self, path: &str) -> std::io::Result<()> {
        let Mode::Record { t, events } = &self.mode else { return Ok(()) };
        let end = events.iter().map(|(at, s, _)| at + s.secs()).fold(*t, f32::max) + 0.5;
        let mut mix = vec![0.0f32; (end * RATE as f32) as usize];
        for (at, s, vol) in events {
            let start = (at * RATE as f32) as usize;
            for (k, &v) in s.data.iter().enumerate() { if let Some(m) = mix.get_mut(start + k) { *m += v as f32 * vol; } }
        }
        let n = mix.len() as u32 * 2;
        let mut out = Vec::with_capacity(44 + n as usize);
        out.extend_from_slice(b"RIFF"); out.extend_from_slice(&(36 + n).to_le_bytes()); out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&RATE.to_le_bytes()); out.extend_from_slice(&(RATE * 2).to_le_bytes()); out.extend_from_slice(&2u16.to_le_bytes()); out.extend_from_slice(&16u16.to_le_bytes());
        out.extend_from_slice(b"data"); out.extend_from_slice(&n.to_le_bytes());
        for m in mix { out.extend_from_slice(&(m.clamp(-32768.0, 32767.0) as i16).to_le_bytes()); }
        std::fs::write(path, out)
    }

    fn send(&mut self, cmd: Cmd, s: &Sample, vol: f32) {
        match &mut self.mode {
            Mode::Off => {}
            Mode::Live { tx, .. } => { let _ = tx.send(cmd); }
            Mode::Record { t, events } => { if let Cmd::Play { .. } = cmd { events.push((*t, s.clone(), vol)); } }
        }
    }
}

impl Drop for Audio {
    fn drop(&mut self) {
        if let Mode::Live { child, .. } = &mut self.mode { let _ = child.kill(); let _ = child.wait(); }
    }
}

struct Voice { data: Arc<Vec<i16>>, pos: f32, step: f32, vol: f32, looped: bool, channel: u8 }

fn start_player() -> Option<(Sender<Cmd>, Child)> {
    let rate = RATE.to_string();
    let attempts: [(&str, Vec<&str>); 3] = [
        ("pw-play", vec!["--raw", "--format=s16", &format!("--rate={}", rate), "--channels=1", "-"].into_iter().map(|s| Box::leak(s.to_string().into_boxed_str()) as &str).collect()),
        ("paplay", vec!["--raw", "--format=s16le", &format!("--rate={}", rate), "--channels=1", "--latency-msec=60", "/dev/stdin"].into_iter().map(|s| Box::leak(s.to_string().into_boxed_str()) as &str).collect()),
        ("aplay", vec!["-q", "-t", "raw", "-f", "S16_LE", "-r", &rate, "-c", "1", "--buffer-time=80000", "-"].into_iter().map(|s| Box::leak(s.to_string().into_boxed_str()) as &str).collect()),
    ];
    let mut child = None;
    for (cmd, args) in &attempts {
        if let Ok(c) = Command::new(cmd).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() { child = Some(c); break; }
    }
    let mut child = child?;
    let mut stdin = child.stdin.take()?;
    let (tx, rx) = channel::<Cmd>();
    std::thread::Builder::new().name("funkey-audio".into()).spawn(move || {
        let mut voices: Vec<Voice> = Vec::new();
        let start = Instant::now();
        let mut written: usize = 0;
        let mut buf = [0u8; CHUNK * 2];
        loop {
            loop {
                match rx.try_recv() {
                    Ok(Cmd::Play { s, vol, looped, channel }) => {
                        if channel != 0 { voices.retain(|v| v.channel != channel); }
                        if voices.len() >= VOICES { voices.remove(0); }
                        voices.push(Voice { data: s.data, pos: 0.0, step: 1.0, vol, looped, channel });
                    }
                    Ok(Cmd::Stop(c)) => voices.retain(|v| v.channel != c),
                    Ok(Cmd::Volume(c, vol)) => for v in voices.iter_mut().filter(|v| v.channel == c) { v.vol = vol; },
                    Ok(Cmd::Rate(c, r)) => for v in voices.iter_mut().filter(|v| v.channel == c) { v.step = r; },
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            // Stay a little ahead of the clock, never seconds ahead.
            let due = (start.elapsed().as_secs_f64() * RATE as f64) as usize + LEAD;
            if written + CHUNK > due { std::thread::sleep(Duration::from_millis(4)); continue; }
            for k in 0..CHUNK {
                let mut acc = 0.0f32;
                for v in voices.iter_mut() {
                    let i = v.pos as usize;
                    if i < v.data.len() { acc += v.data[i] as f32 * v.vol; }
                    v.pos += v.step;
                    if v.looped && v.pos as usize >= v.data.len() { v.pos = 0.0; }
                }
                let s = acc.clamp(-32768.0, 32767.0) as i16;
                buf[k * 2..k * 2 + 2].copy_from_slice(&s.to_le_bytes());
            }
            voices.retain(|v| v.looped || (v.pos as usize) < v.data.len());
            if stdin.write_all(&buf).is_err() { return; }
            written += CHUNK;
        }
    }).ok()?;
    Some((tx, child))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_have_their_pitches_and_tunes_their_lengths() {
        assert!((note_hz("a4") - 440.0).abs() < 0.01);
        assert!((note_hz("c4") - 261.63).abs() < 0.1);
        assert!((note_hz("f#3") - 185.0).abs() < 0.1);
        assert_eq!(note_hz("-"), 0.0);
        let t = Tune::parse("120 c4 e4/8 -/2", Wave::Square, 0.5);
        assert_eq!(t.notes.len(), 3);
        assert!((t.notes[0].1 - 0.5).abs() < 1e-4);
        assert!((t.notes[1].1 - 0.25).abs() < 1e-4);
        assert!((t.notes[2].1 - 1.0).abs() < 1e-4);
        assert!(t.render().secs() > 1.7);
    }

    #[test]
    fn a_wav_round_trips_through_the_recorder() {
        let mut a = Audio::recorder();
        a.play(&Sample::tone(Wave::Sine, 440.0, 0.1, 0.5), 1.0);
        a.set_time(0.5);
        a.play(&Sample::noise(0.1, 0.5), 1.0);
        let p = std::env::temp_dir().join("funkey-audio-test.wav");
        a.write_wav(p.to_str().unwrap()).unwrap();
        let s = Sample::from_wav(&std::fs::read(&p).unwrap()).unwrap();
        assert!(s.secs() > 1.0 && s.data.iter().any(|&v| v != 0));
    }
}
