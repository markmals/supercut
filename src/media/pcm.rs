use std::io::{BufReader, Read};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::thread::JoinHandle;

use anyhow::{Context as _, bail};

/// Interleaved 32-bit float samples decoded by ffmpeg, read incrementally so hour-long episodes
/// never sit in memory whole.
pub struct PcmStream {
    child: Child,
    stdout: BufReader<ChildStdout>,
    stderr: Option<JoinHandle<String>>,
    bytes: Vec<u8>,
}

impl PcmStream {
    /// Decodes `path` resampled to `sample_rate` and mixed to `channels`, optionally only the
    /// `duration` seconds starting at `start`.
    pub fn open(
        path: &Path,
        sample_rate: u32,
        channels: u32,
        start: Option<f64>,
        duration: Option<f64>,
    ) -> anyhow::Result<Self> {
        let mut command = Command::new("ffmpeg");
        command.args(["-nostdin", "-v", "error"]);
        if let Some(start) = start {
            command.arg("-ss").arg(format!("{start:.6}"));
        }
        if let Some(duration) = duration {
            command.arg("-t").arg(format!("{duration:.6}"));
        }
        command
            .arg("-i")
            .arg(path)
            .args(["-map", "0:a:0", "-f", "f32le", "-acodec", "pcm_f32le"])
            .arg("-ar")
            .arg(sample_rate.to_string())
            .arg("-ac")
            .arg(channels.to_string())
            .arg("pipe:1");
        Self::spawn(command)
    }

    /// Runs an ffmpeg `command` that writes `f32le` samples to `pipe:1`.
    pub fn spawn(mut command: Command) -> anyhow::Result<Self> {
        command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        let mut child = command.spawn().context("running ffmpeg; is it installed and on PATH?")?;
        let stdout = BufReader::with_capacity(1 << 16, child.stdout.take().context("ffmpeg stdout")?);
        let mut stderr_pipe = child.stderr.take().context("ffmpeg stderr")?;
        let stderr = std::thread::spawn(move || {
            let mut text = String::new();
            let _ = stderr_pipe.read_to_string(&mut text);
            text
        });
        Ok(Self { child, stdout, stderr: Some(stderr), bytes: Vec::new() })
    }

    /// Fills `samples` from the stream and returns how many were written; 0 means the end.
    pub fn read(&mut self, samples: &mut [f32]) -> anyhow::Result<usize> {
        self.bytes.resize(samples.len() * 4, 0);
        let mut filled = 0;
        while filled < self.bytes.len() {
            let count = self.stdout.read(&mut self.bytes[filled..]).context("reading ffmpeg output")?;
            if count == 0 {
                break;
            }
            filled += count;
        }
        let count = filled / 4;
        for (sample, chunk) in samples.iter_mut().zip(self.bytes[..count * 4].as_chunks::<4>().0) {
            *sample = f32::from_le_bytes(*chunk);
        }
        Ok(count)
    }

    /// Waits for ffmpeg, turns a failed decode into an error, and returns what ffmpeg logged.
    pub fn finish(mut self) -> anyhow::Result<String> {
        let status = self.child.wait().context("waiting for ffmpeg")?;
        let stderr = self.stderr.take().map(|thread| thread.join().unwrap_or_default()).unwrap_or_default();
        if !status.success() {
            bail!("ffmpeg failed to decode ({status}): {}", stderr.trim());
        }
        Ok(stderr)
    }
}

impl Drop for PcmStream {
    fn drop(&mut self) {
        // Still set only when `finish` never ran: stop an abandoned decode.
        if self.stderr.is_some() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}

/// Decodes `duration` seconds from `start` into memory: for short windows only.
pub fn decode_window(path: &Path, start: f64, duration: f64, sample_rate: u32, channels: u32) -> anyhow::Result<Vec<f32>> {
    let mut stream = PcmStream::open(path, sample_rate, channels, Some(start.max(0.0)), Some(duration))?;
    let mut samples = Vec::new();
    let mut chunk = vec![0.0f32; 1 << 15];
    loop {
        let count = stream.read(&mut chunk)?;
        if count == 0 {
            break;
        }
        samples.extend_from_slice(&chunk[..count]);
    }
    stream.finish()?;
    Ok(samples)
}
