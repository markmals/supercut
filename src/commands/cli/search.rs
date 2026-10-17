use clap::CommandFactory as _;
use serde::Serialize;

use crate::commands::Context;
use crate::commands::registry;
use crate::commands::schema::{about, leaf_operations};

/// Find the commands for a task described in plain words.
#[derive(clap::Args)]
pub struct Args {
    /// What you want to do, e.g. "cut the ads out of an episode".
    #[arg(required = true)]
    query: Vec<String>,
    /// Most results to return.
    #[arg(long, default_value_t = 5)]
    limit: usize,
}

#[derive(Debug, Serialize)]
struct Match {
    command: String,
    summary: String,
    score: u32,
    example: &'static str,
}

const STOP_WORDS: &[&str] = &["a", "an", "the", "of", "to", "for", "in", "on", "my", "i", "and", "or", "from", "with", "it", "is", "be", "do", "how", "what", "want", "can", "out"];

pub fn run(_context: &Context, args: Args) -> anyhow::Result<()> {
    let matches = search(&args.query.join(" "), args.limit);
    crate::output::print_json(&matches)
}

fn search(query: &str, limit: usize) -> Vec<Match> {
    let wanted = terms(query);
    let mut matches: Vec<Match> = leaf_operations(&crate::Cli::command())
        .into_iter()
        .map(|(path, command)| {
            let operation = registry::find(&path);
            Match {
                command: format!("supercut {path}"),
                summary: about(&command),
                score: score(&wanted, &path, &command),
                example: operation.map_or("", |operation| operation.example),
            }
        })
        .filter(|candidate| candidate.score > 0)
        .collect();
    matches.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.command.cmp(&b.command)));
    matches.truncate(limit);
    matches
}

/// Each query term scores the weight of the most telling field it appears in.
fn score(wanted: &[String], path: &str, command: &clap::Command) -> u32 {
    let keywords = registry::find(path).map(|operation| operation.keywords.join(" ")).unwrap_or_default();
    let arguments: Vec<String> = command.get_arguments().filter_map(|arg| arg.get_help().map(ToString::to_string)).collect();
    let fields = [(terms(path), 4), (terms(&keywords), 3), (terms(&about(command)), 2), (terms(&arguments.join(" ")), 1)];
    wanted
        .iter()
        .map(|term| fields.iter().filter(|(field, _)| field.contains(term)).map(|(_, weight)| *weight).max().unwrap_or(0))
        .sum()
}

/// Lowercased word stems without stop words, so "transcribing episodes" meets "transcribe episode".
fn terms(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|word| !word.is_empty() && !STOP_WORDS.contains(&word.as_str()))
        .map(|word| stem(&word))
        .collect()
}

fn stem(word: &str) -> String {
    for suffix in ["ing", "ed", "es", "s", "e"] {
        if let Some(stem) = word.strip_suffix(suffix)
            && stem.len() >= 3
        {
            return stem.to_owned();
        }
    }
    word.to_owned()
}

#[cfg(test)]
mod tests {
    use super::search;

    fn top(query: &str) -> String {
        search(query, 1).into_iter().next().map(|found| found.command).unwrap_or_default()
    }

    #[test]
    fn plain_task_descriptions_find_the_right_operation() {
        assert_eq!(top("transcribe an episode"), "supercut transcripts create");
        assert_eq!(top("make the final mp3"), "supercut plans render");
        assert_eq!(top("learn what I cut in my old Logic exports"), "supercut history recover");
        assert_eq!(top("where should I cut, is it silence or music"), "supercut episodes inspect");
        assert_eq!(top("refresh the RSS feed"), "supercut episodes sync");
    }
}
