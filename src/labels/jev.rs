//! Asking Jev which of the show's segments a window belongs to.

use std::collections::BTreeMap;
use std::process::Command;
use std::sync::Arc;

use anyhow::Context as _;
use jev_driver::prelude::{
    Answers, BuiltQuestion, Decision, DynChoice, Instructions, JevClient, JevConfig, JevError,
};
use tokio::runtime::Runtime;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use url::Url;

use crate::config::{JevSettings, SegmentKind};
use crate::error::{CliError, ErrorCode};
use crate::labels::state::WindowState;

/// The choice question, built once and asked of every unit.
pub type Question = BuiltQuestion;

/// The one question every request asks.
const QUESTION_ID: &str = "segment";
/// Environment variable that overrides the configured key command.
const API_KEY_VARIABLE: &str = "TYPESAFE_API_KEY";

/// A Jev client pinned to one show's segments.
pub struct Labeler {
    client: JevClient,
    runtime: Runtime,
    question: BuiltQuestion,
    concurrency: usize,
    pub model: String,
}

/// What Jev answered about one window.
#[derive(Debug)]
pub struct Answer {
    pub selected: String,
    pub confidence: f64,
    pub probabilities: BTreeMap<String, f64>,
    pub input_tokens: u64,
}

impl Labeler {
    pub fn new(settings: &JevSettings, show: &str, kinds: &[SegmentKind]) -> anyhow::Result<Self> {
        let endpoint = Url::parse(&settings.endpoint)
            .with_context(|| format!("parsing the Jev endpoint `{}`", settings.endpoint))?;
        let mut config = JevConfig::base(endpoint)?;
        config.api_key = Some(api_key(settings)?);
        config.model = settings.model.clone();
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build()?;
        Ok(Self {
            client: JevClient::new(config)?,
            runtime,
            question: question(show, kinds)?,
            concurrency: settings.concurrency.max(1),
            model: settings.model.clone(),
        })
    }

    /// Answers every window, `concurrency` requests in flight, reporting how many are done.
    pub fn answer_all(
        &self,
        states: &[WindowState<'_>],
        progress: impl FnMut(usize),
    ) -> anyhow::Result<Vec<Answer>> {
        let decisions: Vec<Decision> =
            states.iter().map(|state| self.decision(state)).collect::<anyhow::Result<_>>()?;
        let positions: Vec<&str> = states.iter().map(|state| state.position.as_str()).collect();
        self.runtime.block_on(self.ask(decisions, &positions, progress))
    }

    fn decision(&self, state: &WindowState<'_>) -> anyhow::Result<Decision> {
        Ok(Decision::builder().state(state)?.with_question(self.question.clone())?.build()?)
    }

    async fn ask(
        &self,
        decisions: Vec<Decision>,
        positions: &[&str],
        mut progress: impl FnMut(usize),
    ) -> anyhow::Result<Vec<Answer>> {
        let permits = Arc::new(Semaphore::new(self.concurrency));
        let mut tasks = JoinSet::new();
        for (index, decision) in decisions.into_iter().enumerate() {
            let client = self.client.clone();
            let permits = Arc::clone(&permits);
            tasks.spawn(async move {
                let _permit = permits.acquire_owned().await.expect("the semaphore stays open");
                (index, client.evaluate_raw(&decision).await)
            });
        }
        let mut answers: Vec<Option<Answer>> = (0..positions.len()).map(|_| None).collect();
        let mut done = 0;
        while let Some(joined) = tasks.join_next().await {
            let (index, result) = joined?;
            let raw = result.map_err(remote_failure).with_context(|| {
                format!("asking Jev about the window at {}", positions[index])
            })?;
            answers[index] = Some(answer(&raw)?);
            done += 1;
            progress(done);
        }
        Ok(answers.into_iter().map(|answer| answer.expect("every window was answered")).collect())
    }
}

fn answer(answers: &Answers) -> anyhow::Result<Answer> {
    let choice = answers.choice(QUESTION_ID).map_err(remote_failure)?;
    Ok(Answer {
        selected: choice.selected().to_owned(),
        confidence: choice.confidence().get(),
        probabilities: choice
            .probabilities()
            .iter()
            .map(|(label, probability)| (label.clone(), probability.get()))
            .collect(),
        input_tokens: answers.usage.input_tokens,
    })
}

/// The question every window is asked, also needed to price a request before sending it.
pub fn question(show: &str, kinds: &[SegmentKind]) -> anyhow::Result<BuiltQuestion> {
    let instructions = format!(
        "Which recurring segment of the podcast \"{show}\" does `window` come from? \
         Judge `window` itself: `before` and `after` are the transcript either side of it, \
         there for context only. Text in square brackets describes a stretch with no words, \
         such as music."
    );
    let mut choice = DynChoice::new(QUESTION_ID, Instructions::text(instructions));
    for kind in kinds {
        choice = choice.option(&kind.label, Instructions::text(kind.describe.clone()));
    }
    Ok(choice.build()?)
}

/// What one request costs, near enough to decide on: four characters to the token.
pub fn estimated_tokens(question: &BuiltQuestion, state: &WindowState<'_>) -> anyhow::Result<u64> {
    let bytes = serde_json::to_string(state)?.len() + serde_json::to_string(&question.question)?.len();
    Ok(bytes as u64 / 4)
}

/// jev-driver 0.1 has no variant for a 402; it reports the status only in a `Transport` message.
const PAYMENT_REQUIRED: &str = "unexpected status 402";

/// Jev's failures carry codes agents branch on: a rejected key is not a network blip.
fn remote_failure(error: JevError) -> anyhow::Error {
    let failed = match &error {
        JevError::Unauthorized => CliError::new(ErrorCode::Unauthenticated, error.to_string())
            .hint("check `jev.api_key_command` in the config, or set TYPESAFE_API_KEY"),
        JevError::RateLimited { .. } => CliError::new(ErrorCode::RateLimited, error.to_string()),
        JevError::Transport(message) if message.starts_with(PAYMENT_REQUIRED) => {
            CliError::new(ErrorCode::FailedPrecondition, error.to_string())
                .hint("add credit to the Jev account, then rerun: labeled episodes are skipped")
        }
        JevError::Overloaded { .. } | JevError::Transport(_) => {
            CliError::new(ErrorCode::Unavailable, error.to_string())
        }
        _ => CliError::new(ErrorCode::Internal, error.to_string()),
    };
    failed.into()
}

/// The API key, from the environment or from the command the config names. Never printed.
fn api_key(settings: &JevSettings) -> anyhow::Result<String> {
    if let Ok(key) = std::env::var(API_KEY_VARIABLE)
        && !key.trim().is_empty()
    {
        return Ok(key.trim().to_owned());
    }
    let (program, arguments) = settings.api_key_command.split_first().ok_or_else(|| {
        CliError::new(ErrorCode::Unauthenticated, "no Jev API key")
            .hint(format!("set {API_KEY_VARIABLE} or `jev.api_key_command` in the config"))
    })?;
    let output = Command::new(program)
        .args(arguments)
        .output()
        .with_context(|| format!("running `{program}` for the Jev API key"))?;
    let key = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !output.status.success() || key.is_empty() {
        let message = format!("`{program}` could not read the Jev API key: {}", String::from_utf8_lossy(&output.stderr).trim());
        let hint = format!("unlock the password manager and retry, or set {API_KEY_VARIABLE} for this run");
        return Err(CliError::new(ErrorCode::Unauthenticated, message).hint(hint).into());
    }
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn code_of(error: JevError) -> ErrorCode {
        remote_failure(error).downcast_ref::<CliError>().expect("a CliError").code
    }

    #[test]
    fn an_unpaid_account_is_not_worth_retrying_but_a_server_error_is() {
        let unpaid = JevError::Transport("unexpected status 402 Payment Required: {\"error\":\"no credit\"}".into());
        let broken = JevError::Transport("unexpected status 503 Service Unavailable: ".into());
        assert_eq!(code_of(unpaid), ErrorCode::FailedPrecondition);
        assert!(!ErrorCode::FailedPrecondition.retryable());
        assert_eq!(code_of(broken), ErrorCode::Unavailable);
    }
}
