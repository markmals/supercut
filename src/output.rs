use serde::Serialize;

/// Prints `value` as pretty JSON on stdout: the result of a data command.
pub fn print_json(value: &impl Serialize) -> anyhow::Result<()> {
    println!("{}", serde_json::to_string_pretty(value)?);
    Ok(())
}
