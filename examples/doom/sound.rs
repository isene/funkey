//! Sound effects from the WAD, mixed here and piped to one audio child
//! (pw-play, paplay or aplay). No music: that needs a synthesizer.

use funkey::wad::Wad;
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Sender, TryRecvError};
use std::sync::Arc;
use std::time::{Duration, Instant};

const RATE: u32 = 11025;
const CHUNK: usize = 256;
const LEAD: usize = 1024;
const VOICES: usize = 8;

pub struct Sound {
    tx: Option<Sender<(Arc<Vec<u8>>, f32)>>,
    child: Option<std::process::Child>,
    lumps: HashMap<String, Arc<Vec<u8>>>,
}

impl Drop for Sound {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(c) = &mut self.child { let _ = c.kill(); let _ = c.wait(); }
    }
}

struct Voice { data: Arc<Vec<u8>>, pos: usize, vol: f32 }

impl Sound {
    pub fn new(wad: &Wad, enabled: bool) -> Sound {
        let mut lumps = HashMap::new();
        for l in &wad.lumps {
            if !l.name.starts_with("DS") || l.data.len() < 8 { continue; }
            let rate = u16::from_le_bytes([l.data[2], l.data[3]]) as u32;
            let count = u32::from_le_bytes([l.data[4], l.data[5], l.data[6], l.data[7]]) as usize;
            let body = &l.data[8..];
            let n = count.min(body.len());
            let samples = if n > 48 { &body[16..n - 16] } else { &body[..n] };
            let data: Vec<u8> = if rate >= RATE * 2 { samples.iter().step_by(2).copied().collect() } else { samples.to_vec() };
            lumps.insert(l.name[2..].to_string(), Arc::new(data));
        }
        let (tx, child) = if enabled { start_player().map(|(t, c)| (Some(t), Some(c))).unwrap_or((None, None)) } else { (None, None) };
        Sound { tx, child, lumps }
    }

    /// Mix a list of (tic, name, volume) into a WAV file: the sound track
    /// of a scripted run.
    pub fn render_wav(&self, events: &[(i32, String, f32)], path: &str) -> std::io::Result<()> {
        let last = events.iter().map(|e| e.0).max().unwrap_or(0) as usize + 70;
        let mut mix = vec![0.0f32; last * RATE as usize / 35];
        for (tic, name, vol) in events {
            let Some(d) = self.lumps.get(name) else { continue };
            let at = *tic as usize * RATE as usize / 35;
            for (k, &s) in d.iter().enumerate() {
                if let Some(m) = mix.get_mut(at + k) { *m += (s as f32 - 128.0) * vol; }
            }
        }
        let mut out = Vec::with_capacity(44 + mix.len());
        let n = mix.len() as u32;
        out.extend_from_slice(b"RIFF"); out.extend_from_slice(&(36 + n).to_le_bytes()); out.extend_from_slice(b"WAVEfmt ");
        out.extend_from_slice(&16u32.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&RATE.to_le_bytes()); out.extend_from_slice(&RATE.to_le_bytes()); out.extend_from_slice(&1u16.to_le_bytes()); out.extend_from_slice(&8u16.to_le_bytes());
        out.extend_from_slice(b"data"); out.extend_from_slice(&n.to_le_bytes());
        out.extend(mix.iter().map(|m| (m.clamp(-128.0, 127.0) + 128.0) as u8));
        std::fs::write(path, out)
    }

    pub fn play(&self, name: &str, vol: f32) {
        let Some(tx) = &self.tx else { return };
        let Some(d) = self.lumps.get(name) else { return };
        let _ = tx.send((d.clone(), vol.clamp(0.0, 1.0)));
    }
}

fn start_player() -> Option<(Sender<(Arc<Vec<u8>>, f32)>, std::process::Child)> {
    let attempts: [(&str, &[&str]); 3] = [
        ("pw-play", &["--raw", "--format=u8", "--rate=11025", "--channels=1", "-"]),
        ("paplay", &["--raw", "--format=u8", "--rate=11025", "--channels=1", "--latency-msec=60", "/dev/stdin"]),
        ("aplay", &["-q", "-t", "raw", "-f", "U8", "-r", "11025", "-c", "1", "--buffer-time=80000", "-"]),
    ];
    let mut child = None;
    for (cmd, args) in attempts {
        if let Ok(c) = Command::new(cmd).args(args).stdin(Stdio::piped()).stdout(Stdio::null()).stderr(Stdio::null()).spawn() {
            child = Some(c);
            break;
        }
    }
    let mut child = child?;
    let mut stdin = child.stdin.take()?;
    let (tx, rx) = channel::<(Arc<Vec<u8>>, f32)>();
    std::thread::Builder::new().name("doom-sound".into()).spawn(move || {
        let mut voices: Vec<Voice> = Vec::new();
        let start = Instant::now();
        let mut written: usize = 0;
        let mut buf = [128u8; CHUNK];
        loop {
            loop {
                match rx.try_recv() {
                    Ok((data, vol)) => {
                        if voices.len() >= VOICES { voices.remove(0); }
                        voices.push(Voice { data, pos: 0, vol });
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            // Keep a small lead over the clock, so the pipe never fills up
            // with seconds of latency.
            let due = (start.elapsed().as_secs_f64() * RATE as f64) as usize + LEAD;
            if written + CHUNK > due {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
            for (k, out) in buf.iter_mut().enumerate() {
                let mut acc = 0.0f32;
                for v in &voices {
                    if let Some(&s) = v.data.get(v.pos + k) { acc += (s as f32 - 128.0) * v.vol; }
                }
                *out = (acc.clamp(-128.0, 127.0) + 128.0) as u8;
            }
            for v in voices.iter_mut() { v.pos += CHUNK; }
            voices.retain(|v| v.pos < v.data.len());
            if stdin.write_all(&buf).is_err() { return; }
            written += CHUNK;
        }
    }).ok()?;
    Some((tx, child))
}
