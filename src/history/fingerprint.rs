use std::path::Path;

use anyhow::Context as _;
use rusty_chromaprint::{Configuration, Fingerprinter};

use crate::media::PcmStream;

/// Chromaprint's own working rate; decoding straight to it skips its resampler.
const SAMPLE_RATE: u32 = 11_025;

pub fn configuration() -> Configuration {
    Configuration::preset_test2()
}

/// Seconds between consecutive fingerprint items.
pub fn item_seconds() -> f64 {
    f64::from(configuration().item_duration_in_seconds())
}

/// The fingerprint cached at `cache`, computed from `audio` when absent.
pub fn cached(audio: &Path, cache: &Path) -> anyhow::Result<Vec<u32>> {
    if cache.exists() {
        let bytes = std::fs::read(cache).with_context(|| format!("reading {}", cache.display()))?;
        return Ok(bytes.chunks_exact(4).map(|chunk| u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]])).collect());
    }
    eprintln!("fingerprinting {}", audio.display());
    let fingerprint = compute(audio)?;
    let bytes: Vec<u8> = fingerprint.iter().flat_map(|item| item.to_le_bytes()).collect();
    crate::library::write_atomically(cache, &bytes)?;
    Ok(fingerprint)
}

fn compute(audio: &Path) -> anyhow::Result<Vec<u32>> {
    let config = configuration();
    let mut printer = Fingerprinter::new(&config);
    printer.start(SAMPLE_RATE, 1).context("starting chromaprint")?;
    let mut stream = PcmStream::open(audio, SAMPLE_RATE, 1, None, None)?;
    let mut samples = vec![0.0f32; 1 << 16];
    let mut pcm = Vec::with_capacity(samples.len());
    loop {
        let count = stream.read(&mut samples)?;
        if count == 0 {
            break;
        }
        pcm.clear();
        pcm.extend(samples[..count].iter().map(|sample| (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16));
        printer.consume(&pcm);
    }
    stream.finish()?;
    printer.finish();
    Ok(printer.fingerprint().to_vec())
}
