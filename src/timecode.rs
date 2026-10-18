use std::fmt;
use std::str::FromStr;

use anyhow::{Context as _, bail};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A position or duration in seconds, written as `h:mm:ss.sss`, `m:ss.sss` or plain seconds.
#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Time(pub f64);

impl Time {
    pub fn seconds(self) -> f64 {
        self.0
    }
}

impl fmt::Display for Time {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let negative = self.0 < 0.0;
        let total_millis = (self.0.abs() * 1000.0).round() as u64;
        let millis = total_millis % 1000;
        let total_seconds = total_millis / 1000;
        let (hours, minutes, seconds) =
            (total_seconds / 3600, total_seconds / 60 % 60, total_seconds % 60);
        let sign = if negative { "-" } else { "" };
        if hours > 0 {
            write!(f, "{sign}{hours}:{minutes:02}:{seconds:02}.{millis:03}")
        } else {
            write!(f, "{sign}{minutes}:{seconds:02}.{millis:03}")
        }
    }
}

impl FromStr for Time {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> anyhow::Result<Self> {
        let text = text.trim();
        let (negative, unsigned) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        let parts: Vec<&str> = unsigned.split(':').collect();
        if parts.len() > 3 || parts.iter().any(|part| part.is_empty()) {
            bail!("invalid time `{text}`: expected h:mm:ss.sss, m:ss.sss or seconds");
        }
        let mut seconds = 0.0;
        for part in &parts {
            let value: f64 = part.parse().with_context(|| format!("invalid time `{text}`"))?;
            seconds = seconds * 60.0 + value;
        }
        if !seconds.is_finite() {
            bail!("invalid time `{text}`: not a finite number of seconds");
        }
        Ok(Time(if negative { -seconds } else { seconds }))
    }
}

impl Serialize for Time {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Time {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Seconds(f64),
            Text(String),
        }
        match Raw::deserialize(deserializer)? {
            Raw::Seconds(seconds) => Ok(Time(seconds)),
            Raw::Text(text) => text.parse().map_err(serde::de::Error::custom),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Time;

    #[test]
    fn parses_every_accepted_form() {
        assert_eq!("75.5".parse::<Time>().unwrap(), Time(75.5));
        assert_eq!("1:15.5".parse::<Time>().unwrap(), Time(75.5));
        assert_eq!("1:01:15.5".parse::<Time>().unwrap(), Time(3675.5));
        assert!("1::2".parse::<Time>().is_err());
        assert!("1:2:3:4".parse::<Time>().is_err());
        assert!("NaN".parse::<Time>().is_err(), "a plan time must be a real position");
        assert!("1:inf".parse::<Time>().is_err());
    }

    #[test]
    fn formats_round_trip_to_the_millisecond() {
        for seconds in [0.0, 59.9996, 3599.9994, 3675.5, 36_000.125] {
            let text = Time(seconds).to_string();
            let parsed: Time = text.parse().unwrap();
            assert!((parsed.0 - seconds).abs() <= 0.0005, "{seconds} -> {text}");
        }
        assert_eq!(Time(3675.5).to_string(), "1:01:15.500");
        assert_eq!(Time(59.9996).to_string(), "1:00.000");
    }
}
