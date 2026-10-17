use clap::{ArgAction, CommandFactory as _};
use serde::Serialize;

use crate::commands::Context;
use crate::commands::registry::{self, Effect};
use crate::error::CliError;

/// Describe an operation as JSON: its arguments, side effects, output and an example.
#[derive(clap::Args)]
pub struct Args {
    /// Operation path, e.g. `plans render`. Omit to list every operation with its summary.
    path: Vec<String>,
}

#[derive(Serialize)]
pub struct Summary {
    pub command: String,
    pub summary: String,
}

#[derive(Serialize)]
struct OperationSchema {
    command: String,
    summary: String,
    description: Option<String>,
    usage: String,
    arguments: Vec<ArgumentSchema>,
    dry_run: bool,
    effects: &'static [Effect],
    output: &'static str,
    example: &'static str,
}

#[derive(Serialize)]
struct ArgumentSchema {
    name: String,
    kind: &'static str,
    required: bool,
    multiple: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    default: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    values: Vec<String>,
    help: String,
}

pub fn run(_context: &Context, args: Args) -> anyhow::Result<()> {
    let root = crate::Cli::command();
    let operations = leaf_operations(&root);
    if args.path.is_empty() {
        let summaries: Vec<Summary> =
            operations.iter().map(|(path, command)| Summary { command: format!("supercut {path}"), summary: about(command) }).collect();
        return crate::output::print_json(&summaries);
    }
    let wanted = args.path.join(" ");
    let (path, command) = operations.iter().find(|(path, _)| *path == wanted).ok_or_else(|| {
        CliError::not_found(format!("no operation `{wanted}`")).hint(format!("supercut cli search \"{wanted}\""))
    })?;
    crate::output::print_json(&describe(path, command))
}

/// Every runnable operation as (`resource operation`, its clap command).
pub fn leaf_operations(root: &clap::Command) -> Vec<(String, clap::Command)> {
    let mut found = Vec::new();
    collect_leaves(root, "", &mut found);
    found
}

fn collect_leaves(command: &clap::Command, prefix: &str, found: &mut Vec<(String, clap::Command)>) {
    for child in command.get_subcommands().filter(|child| child.get_name() != "help") {
        let path = if prefix.is_empty() { child.get_name().to_owned() } else { format!("{prefix} {}", child.get_name()) };
        if child.has_subcommands() {
            collect_leaves(child, &path, found);
        } else {
            found.push((path, child.clone()));
        }
    }
}

pub fn about(command: &clap::Command) -> String {
    command.get_about().map(ToString::to_string).unwrap_or_default()
}

fn describe(path: &str, command: &clap::Command) -> OperationSchema {
    let annotations = registry::find(path);
    let arguments: Vec<ArgumentSchema> = command
        .get_arguments()
        .filter(|arg| !matches!(arg.get_id().as_str(), "help" | "version" | "config"))
        .map(describe_argument)
        .collect();
    OperationSchema {
        command: format!("supercut {path}"),
        summary: about(command),
        description: command.get_long_about().map(ToString::to_string),
        usage: command.clone().bin_name(format!("supercut {path}")).render_usage().to_string(),
        dry_run: arguments.iter().any(|argument| argument.name == "--dry-run"),
        arguments,
        effects: annotations.map_or(&[], |operation| operation.effects),
        output: annotations.map_or("", |operation| operation.output),
        example: annotations.map_or("", |operation| operation.example),
    }
}

fn describe_argument(arg: &clap::Arg) -> ArgumentSchema {
    let flag = matches!(arg.get_action(), ArgAction::SetTrue | ArgAction::SetFalse | ArgAction::Count);
    let name = match (arg.get_long(), arg.is_positional()) {
        (Some(long), _) => format!("--{long}"),
        (None, _) => arg.get_value_names().and_then(|names| names.first()).map_or_else(|| arg.get_id().to_string(), ToString::to_string),
    };
    ArgumentSchema {
        name,
        kind: if arg.is_positional() { "positional" } else if flag { "flag" } else { "option" },
        required: arg.is_required_set(),
        multiple: matches!(arg.get_action(), ArgAction::Append) || arg.get_num_args().is_some_and(|range| range.max_values() > 1),
        default: if flag { Vec::new() } else { arg.get_default_values().iter().map(|value| value.to_string_lossy().into_owned()).collect() },
        values: arg.get_possible_values().iter().map(|value| value.get_name().to_owned()).collect(),
        help: arg.get_help().map(ToString::to_string).unwrap_or_default(),
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory as _;

    use super::leaf_operations;
    use crate::commands::registry::OPERATIONS;

    #[test]
    fn registry_and_cli_describe_the_same_operations() {
        let mut cli: Vec<String> = leaf_operations(&crate::Cli::command()).into_iter().map(|(path, _)| path).collect();
        let mut registered: Vec<String> = OPERATIONS.iter().map(|operation| operation.path.to_owned()).collect();
        cli.sort();
        registered.sort();
        assert_eq!(cli, registered);
    }

    #[test]
    fn every_registered_example_parses() {
        for operation in OPERATIONS {
            let words = shell_words(operation.example);
            let parsed = crate::Cli::command().try_get_matches_from(&words);
            assert!(parsed.is_ok(), "`{}` does not parse: {:?}", operation.example, parsed.err());
        }
    }

    fn shell_words(example: &str) -> Vec<String> {
        let mut words = Vec::new();
        let mut current = String::new();
        let mut quoted = false;
        for character in example.chars() {
            match character {
                '"' => quoted = !quoted,
                ' ' if !quoted => words.push(std::mem::take(&mut current)),
                other => current.push(other),
            }
        }
        words.push(current);
        words
    }
}
