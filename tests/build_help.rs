//! `build` is an addition that must not change what `--help` already says.

mod common;

use common::TestResult;
use common::cli::invoke;

#[test]
fn the_top_level_help_still_lists_exactly_the_original_commands() -> TestResult {
    let run = invoke(&["--help"])?;
    let lines: Vec<&str> = run.err.lines().collect();
    let start = lines.iter().position(|l| *l == "Commands:").expect("Commands section");
    let commands: Vec<&str> = lines[start + 1..]
        .iter()
        .copied()
        .take_while(|l| !l.is_empty())
        .filter_map(|l| l.split_whitespace().next())
        .collect();
    assert_eq!(commands, ["preview", "inspect", "texture", "export", "help"], "{}", run.err);
    assert!(!run.err.contains("build"), "{}", run.err);
    Ok(())
}

#[test]
fn the_hidden_build_command_still_has_its_own_help() -> TestResult {
    let run = invoke(&["build", "--help"])?;
    assert!(run.err.contains("Usage: voxforge build <DOCUMENT>"), "{}", run.err);
    assert!(!run.err.contains("Hidden from"), "{}", run.err);
    Ok(())
}
