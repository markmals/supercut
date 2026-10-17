use std::ffi::OsString;
use std::io::{BufRead, BufReader, IsTerminal as _, Write as _};
use std::process::{Command, Stdio};

use anyhow::Context as _;

use crate::error::{CliError, ErrorCode};

/// Runs ffmpeg with `args`, turning its progress stream into one line of stderr.
pub fn run(args: &[OsString], total_seconds: f64, label: &str) -> anyhow::Result<()> {
    let mut child = Command::new("ffmpeg")
        .args(["-progress", "pipe:1", "-nostats"])
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|error| {
            anyhow::Error::new(error)
                .context(CliError::new(ErrorCode::ExternalTool, "could not run ffmpeg").hint("install ffmpeg"))
        })?;

    let progress = child.stdout.take().expect("stdout is piped");
    report(BufReader::new(progress), total_seconds, label);

    let status = child.wait().context("waiting for ffmpeg")?;
    if status.success() {
        return Ok(());
    }
    let failure = CliError::new(ErrorCode::ExternalTool, format!("ffmpeg failed while {label} ({status})"))
        .hint("ffmpeg logged the reason on stderr");
    Err(failure.into())
}

/// ffmpeg writes `key=value` lines; `out_time_us` says how much audio it has written. A
/// terminal gets one updating line; a log gets a line every 10%.
fn report(progress: impl BufRead, total_seconds: f64, label: &str) {
    let terminal = std::io::stderr().is_terminal();
    let mut percent = 0;
    for line in progress.lines().map_while(Result::ok) {
        let Some(microseconds) = line.strip_prefix("out_time_us=").and_then(|value| value.parse::<i64>().ok()) else {
            continue;
        };
        let done = (microseconds as f64 / 1_000_000.0 / total_seconds * 100.0).clamp(0.0, 100.0) as u32;
        if done == percent {
            continue;
        }
        percent = done;
        if terminal {
            eprint!("\r{label}: {percent}%");
            let _ = std::io::stderr().flush();
        } else if percent % 10 == 0 {
            eprintln!("{label}: {percent}%");
        }
    }
    eprintln!("{}{label}: done", if terminal { "\r" } else { "" });
}
