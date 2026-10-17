use std::ffi::OsString;
use std::path::Path;

use crate::render::format::OutputFormat;
use crate::render::piece::{BEEP_SECONDS, Piece, Source};

/// Level the preview marker plays at: a bookmark between transitions, not a shock.
const BEEP_LEVEL_DB: f64 = -30.0;
/// ffmpeg's `sine` source peaks at an eighth of full scale, whatever the format.
const SINE_PEAK_DB: f64 = -18.06;
const BEEP_TONE_HZ: u32 = 660;
const BEEP_TONE_SECONDS: f64 = 0.15;
const BEEP_LEAD_MS: u32 = 350;

/// ffmpeg arguments that render `pieces` into `output` as MP3.
///
/// Every piece is its own input with its own seek: ffmpeg's input seeking is sample accurate when
/// it transcodes, and one input per piece keeps hour-long episodes streaming instead of buffered.
pub fn encode_args(pieces: &[Piece], format: &OutputFormat, limit_db: f64, output: &Path) -> Vec<OsString> {
    let mut args: Vec<OsString> =
        ["-nostdin", "-hide_banner", "-v", "error", "-y"].into_iter().map(OsString::from).collect();
    for piece in pieces {
        push_input(&mut args, piece, format);
    }
    args.push("-filter_complex".into());
    args.push(filter_graph(pieces, format, limit_db).into());
    for argument in ["-map", "[out]", "-map_metadata", "-1", "-c:a", "libmp3lame", "-b:a"] {
        args.push(argument.into());
    }
    args.push(format.bit_rate_arg().into());
    args.push("-f".into());
    args.push("mp3".into());
    args.push(output.as_os_str().to_owned());
    args
}

fn push_input(args: &mut Vec<OsString>, piece: &Piece, format: &OutputFormat) {
    match &piece.source {
        Source::File { path, start, end } => {
            args.push("-ss".into());
            args.push(format!("{start:.6}").into());
            args.push("-to".into());
            args.push(format!("{end:.6}").into());
            args.push("-i".into());
            args.push(path.as_os_str().to_owned());
        }
        Source::Beep => {
            args.push("-f".into());
            args.push("lavfi".into());
            args.push("-i".into());
            args.push(
                format!(
                    "sine=frequency={BEEP_TONE_HZ}:duration={BEEP_TONE_SECONDS}:sample_rate={}",
                    format.sample_rate
                )
                .into(),
            );
        }
    }
}

fn filter_graph(pieces: &[Piece], format: &OutputFormat, limit_db: f64) -> String {
    let mut chains: Vec<String> = pieces
        .iter()
        .enumerate()
        .map(|(index, piece)| format!("[{index}:a:0]{}[p{index}]", piece_filters(piece, format).join(",")))
        .collect();
    let inputs: String = (0..pieces.len()).map(|index| format!("[p{index}]")).collect();
    chains.push(format!("{inputs}concat=n={}:v=0:a=1[joined]", pieces.len()));
    chains.push(format!("[joined]alimiter=limit={:.6}:level=false:latency=true[out]", linear(limit_db)));
    chains.join(";")
}

fn piece_filters(piece: &Piece, format: &OutputFormat) -> Vec<String> {
    let mut filters = vec!["asetpts=PTS-STARTPTS".to_string()];
    if matches!(piece.source, Source::Beep) {
        filters.extend(beep_filters());
    }
    filters.push(format!("aresample={}", format.sample_rate));
    filters.push(format!("aformat=sample_fmts=fltp:channel_layouts={}", format.layout()));
    if piece.gain_db.abs() >= 0.01 {
        filters.push(format!("volume={:.2}dB", piece.gain_db));
    }
    if piece.fade_in > 0.0 {
        filters.push(format!("afade=t=in:st=0:d={:.3}", piece.fade_in));
    }
    if piece.fade_out > 0.0 {
        filters.push(format!("afade=t=out:st={:.3}:d={:.3}", piece.seconds() - piece.fade_out, piece.fade_out));
    }
    filters
}

/// Pads the generated tone with silence so the marker lasts `BEEP_SECONDS`.
fn beep_filters() -> Vec<String> {
    let tail = BEEP_SECONDS - BEEP_TONE_SECONDS - f64::from(BEEP_LEAD_MS) / 1000.0;
    vec![
        format!("volume={:.2}dB", BEEP_LEVEL_DB - SINE_PEAK_DB),
        format!("adelay={BEEP_LEAD_MS}:all=1"),
        format!("apad=pad_dur={tail:.3}"),
    ]
}

/// Amplitude for a level in dB, inside the limiter's range.
fn linear(db: f64) -> f64 {
    10f64.powf(db / 20.0).clamp(0.0625, 1.0)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{encode_args, linear};
    use crate::render::format::OutputFormat;
    use crate::render::piece::{Piece, Source};

    fn format() -> OutputFormat {
        OutputFormat { sample_rate: 44_100, channels: 2, bit_rate: 128_000 }
    }

    #[test]
    fn seeks_each_piece_on_its_own_input() {
        let pieces = vec![
            Piece::new(Source::file("a.mp3", 0.0, 10.0), -2.0, 0.0).faded(0.0, 0.01),
            Piece::new(Source::file("a.mp3", 20.0, 30.0), -2.0, 10.0).faded(0.01, 0.0),
        ];
        let args: Vec<String> = encode_args(&pieces, &format(), -1.5, Path::new("out.mp3.partial"))
            .iter()
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        let joined = args.join(" ");
        assert!(joined.contains("-ss 0.000000 -to 10.000000 -i a.mp3"));
        assert!(joined.contains("-ss 20.000000 -to 30.000000 -i a.mp3"));
        assert!(joined.contains("afade=t=out:st=9.990:d=0.010"));
        assert!(joined.contains("afade=t=in:st=0:d=0.010"));
        assert!(joined.contains("concat=n=2:v=0:a=1"));
        assert_eq!(args.last().unwrap(), "out.mp3.partial");
    }

    #[test]
    fn limiter_ceiling_is_an_amplitude() {
        assert!((linear(0.0) - 1.0).abs() < 1e-9);
        assert!((linear(-1.5) - 0.8413).abs() < 1e-4);
    }
}
