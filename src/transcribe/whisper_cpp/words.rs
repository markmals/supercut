//! Gluing whisper.cpp's sub-word tokens back into words.

use anyhow::ensure;

use crate::transcript::Word;

/// The words of one segment. Whisper emits sub-word tokens; a leading space marks the start of
/// the next word, and each word runs until the one after it starts.
pub fn of(segment: &super::Segment) -> anyhow::Result<Vec<Word>> {
    let mut started: Vec<Started> = Vec::new();
    for token in segment.tokens.iter().filter(|token| !is_special(&token.text)) {
        ensure!(
            token.t_dtw >= 0.0,
            "whisper-cli timed no token in `{}`; the model may not support `-dtw`",
            segment_text(segment)
        );
        match started.last_mut() {
            Some(current) if !token.text.starts_with(' ') => current.take(token),
            _ => started.push(Started::new(token)),
        }
    }
    let segment_end = segment.offsets.to / 1000.0;
    Ok(started
        .iter()
        .enumerate()
        .filter_map(|(position, word)| {
            let end = started.get(position + 1).map_or(segment_end, |next| next.start);
            super::super::word_of(&word.text, word.start, end, word.probability())
        })
        .collect())
}

/// Timestamps and other control tokens whisper.cpp spells out, such as `[_BEG_]` and `[_TT_465]`.
fn is_special(text: &str) -> bool {
    text.starts_with("[_")
}

fn segment_text(segment: &super::Segment) -> String {
    segment.tokens.iter().map(|token| token.text.as_str()).collect()
}

/// A word while its tokens are still arriving.
struct Started {
    text: String,
    start: f64,
    probabilities: Vec<f32>,
}

impl Started {
    fn new(token: &super::Token) -> Self {
        let mut word = Self { text: String::new(), start: token.t_dtw / 100.0, probabilities: Vec::new() };
        word.take(token);
        word
    }

    fn take(&mut self, token: &super::Token) {
        self.text.push_str(&token.text);
        self.probabilities.extend(token.p);
    }

    fn probability(&self) -> Option<f32> {
        match self.probabilities.len() {
            0 => None,
            count => Some(self.probabilities.iter().sum::<f32>() / count as f32),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::transcribe::whisper_cpp::tests::config;

    fn token(text: &str, dtw: i32, p: f32) -> String {
        format!(r#"{{"text": "{text}", "offsets": {{"from": 0, "to": 0}}, "id": 1, "p": {p}, "t_dtw": {dtw}}}"#)
    }

    /// Tokens as whisper.cpp wrote them for "Hi, this is Holly." in the smoke-test clip.
    fn report(tokens: &[String]) -> String {
        format!(
            r#"{{"transcription": [{{"offsets": {{"from": 24680, "to": 26620}}, "tokens": [{}]}}]}}"#,
            tokens.join(",")
        )
    }

    #[test]
    fn glues_sub_word_tokens_into_words_that_run_until_the_next_one() {
        let json = report(&[
            token("[_BEG_]", -1, 0.58),
            token(" Hi", 2500, 0.4),
            token(",", 2520, 0.6),
            token(" this", 2530, 1.0),
            token(" is", 2546, 1.0),
            token(" Holly", 2574, 1.0),
            token(".", 2580, 1.0),
        ]);
        let transcript = crate::transcribe::whisper_cpp::parse(&json, &config()).unwrap();
        let said: Vec<(f64, f64, &str)> =
            transcript.words().map(|word| (word.start, word.end, word.text.as_str())).collect();
        assert_eq!(
            said,
            [(25.0, 25.3, "Hi,"), (25.3, 25.46, "this"), (25.46, 25.74, "is"), (25.74, 26.62, "Holly.")]
        );
        assert_eq!(transcript.segments[0].words[0].probability, Some(0.5));
        assert_eq!(transcript.segments[0].text, "Hi, this is Holly.");
    }

    #[test]
    fn refuses_tokens_the_engine_never_timed() {
        let json = report(&[token(" Hi", -1, 0.4)]);
        let error = crate::transcribe::whisper_cpp::parse(&json, &config()).unwrap_err();
        assert!(format!("{error:#}").contains("-dtw"), "{error:#}");
    }
}
