use anyhow::Context as _;

use crate::media::Probe;

/// The bit rates an MP3 frame can carry, in bits per second.
const MP3_BIT_RATES: [u64; 14] = [
    32_000, 40_000, 48_000, 56_000, 64_000, 80_000, 96_000, 112_000, 128_000, 160_000, 192_000, 224_000, 256_000,
    320_000,
];

/// What the rendered file is encoded as: the sources' own format, so nothing is resampled or
/// re-rated more than the splice itself needs.
#[derive(Clone, Copy, Debug)]
pub struct OutputFormat {
    pub sample_rate: u32,
    pub channels: u32,
    pub bit_rate: u64,
}

impl OutputFormat {
    /// The first episode's rate and channels at the highest bit rate any episode was encoded at.
    pub fn from_probes(probes: &[Probe]) -> anyhow::Result<Self> {
        let first = probes.first().context("a plan needs at least one episode")?;
        let bit_rate = probes
            .iter()
            .filter_map(|probe| probe.bit_rate)
            .max()
            .context("no episode states its bit rate; cannot pick the output bit rate")?;
        Ok(Self { sample_rate: first.sample_rate, channels: first.channels.clamp(1, 2), bit_rate: snap(bit_rate) })
    }

    pub fn layout(&self) -> &'static str {
        if self.channels == 1 { "mono" } else { "stereo" }
    }

    pub fn bit_rate_arg(&self) -> String {
        format!("{}", self.bit_rate)
    }
}

/// The standard MP3 bit rate closest to `bit_rate`.
fn snap(bit_rate: u64) -> u64 {
    MP3_BIT_RATES
        .into_iter()
        .min_by_key(|standard| standard.abs_diff(bit_rate))
        .unwrap_or(*MP3_BIT_RATES.last().unwrap())
}

#[cfg(test)]
mod tests {
    use super::{OutputFormat, snap};
    use crate::media::Probe;

    fn probe(sample_rate: u32, channels: u32, bit_rate: Option<u64>) -> Probe {
        Probe { codec: "mp3".into(), sample_rate, channels, bit_rate, duration: 60.0 }
    }

    #[test]
    fn snaps_measured_bit_rates_to_mp3_rates() {
        assert_eq!(snap(128_000), 128_000);
        assert_eq!(snap(131_200), 128_000);
        assert_eq!(snap(400_000), 320_000);
    }

    #[test]
    fn takes_rate_and_channels_from_the_first_episode_and_the_highest_bit_rate() {
        let format =
            OutputFormat::from_probes(&[probe(44_100, 2, Some(128_000)), probe(48_000, 1, Some(192_000))]).unwrap();
        assert_eq!((format.sample_rate, format.channels, format.bit_rate), (44_100, 2, 192_000));
    }

    #[test]
    fn refuses_sources_without_a_bit_rate() {
        assert!(OutputFormat::from_probes(&[probe(44_100, 2, None)]).is_err());
    }
}
