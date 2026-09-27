//! Derived CLI definition covering dispatch between a declared subcommand
//! (`run`, aliased `r`) and plugin-provided external subcommands. Global
//! options (`--config`, repeatable `--define`) are only parsed before the
//! subcommand name; everything afterwards is captured verbatim. Results are
//! observed solely through `Cli::try_parse_from` with argv0 included.
#![allow(unreachable_pub)] // types stay observable to sibling test modules

use clap::Args;
use clap::Parser;
use clap::Subcommand;

fn parse_define(raw: &str) -> Result<String, String> {
    if raw.contains('=') {
        Ok(raw.to_owned())
    } else {
        Err(format!("invalid define `{raw}`: expected key=value"))
    }
}

#[derive(Parser, Debug, PartialEq)]
#[command(
    name = "tool",
    about = "Root command dispatching to built-in and plugin subcommands",
    override_usage = "tool [OPTIONS] <COMMAND>"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "builtin.toml", value_name = "path")]
    pub config: String,
    #[arg(long, global = true, value_name = "key=value", value_parser = parse_define)]
    pub define: Vec<String>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    #[command(about = "Run a job", alias = "r")]
    Run(RunArgs),
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args, Debug, PartialEq)]
#[command(override_usage = "tool run [OPTIONS] --output <path> [-- <TRAILING>...]")]
pub struct RunArgs {
    #[arg(long, value_name = "path", required = true)]
    pub output: String,
    #[arg(last = true)]
    pub trailing: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    fn parse(argv: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(argv)
    }

    fn external_ok(argv: &[&str]) -> (String, Vec<String>, Vec<String>) {
        let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
        let Commands::External(tokens) = cli.command else {
            panic!("expected external subcommand for {argv:?}");
        };
        assert!(
            !tokens.is_empty(),
            "external capture must always include the command name: {argv:?}"
        );
        (cli.config, cli.define, tokens)
    }

    fn run_ok(argv: &[&str]) -> (String, Vec<String>, RunArgs) {
        let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
        let Commands::Run(run) = &cli.command else {
            panic!("expected `run` parse for {argv:?}");
        };
        let run = RunArgs {
            output: run.output.clone(),
            trailing: run.trailing.clone(),
        };
        (cli.config, cli.define, run)
    }

    #[test]
    fn run_requires_output_and_uses_defaults() {
        let (config, define, run) = run_ok(&["tool", "run", "--output", "out.txt"]);
        assert_eq!(config, "builtin.toml");
        assert_eq!(define, Vec::<String>::new());
        assert_eq!(run.output, "out.txt");
        assert_eq!(run.trailing, Vec::<String>::new());
    }

    #[test]
    fn run_alias_r_enters_declared_command() {
        let (_, _, run) = run_ok(&["tool", "r", "--output", "out.txt"]);
        assert_eq!(run.output, "out.txt");

        let (_, _, run) = run_ok(&[
            "tool", "r", "--output", "out.txt", "--", "a", "--flag",
        ]);
        assert_eq!(run.output, "out.txt");
        assert_eq!(run.trailing, vec!["a", "--flag"]);
    }

    #[test]
    fn run_dispatch_validates_output_for_both_names() {
        for argv in [&["tool", "run"][..], &["tool", "r"][..]] {
            let err = parse(argv).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument, "{argv:?}");
            let msg = err.to_string();
            assert!(msg.contains("--output"), "{argv:?}: {msg}");
            assert!(msg.contains("tool run"), "{argv:?}: {msg}");
        }
    }

    #[test]
    fn run_prefix_is_external_not_inferred_subcommand() {
        let (_, _, tokens) = external_ok(&["tool", "ru", "--output", "x"]);
        assert_eq!(tokens, vec!["ru", "--output", "x"]);
    }

    #[test]
    fn run_trailing_after_separator_is_raw_and_stays_in_run() {
        let (config, define, run) = run_ok(&[
            "tool",
            "run",
            "--output",
            "out.txt",
            "--",
            "plug",
            "--config",
            "other.toml",
            "run",
            "--",
            "",
        ]);
        assert_eq!(run.output, "out.txt");
        assert_eq!(
            run.trailing,
            vec!["plug", "--config", "other.toml", "run", "--", ""]
        );
        // Tokens after run's separator must not be reinterpreted as root state.
        assert_eq!(config, "builtin.toml");
        assert_eq!(define, Vec::<String>::new());
    }

    #[test]
    fn run_rejects_positional_before_separator() {
        let err = parse(&["tool", "run", "--output", "o", "extra"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        assert!(err.to_string().contains("extra"));
    }

    #[test]
    fn external_command_captures_name_and_all_following_tokens() {
        let (_, _, tokens) = external_ok(&[
            "tool", "plug", "--flag", "run", "--output", "x", "r",
        ]);
        assert_eq!(
            tokens,
            vec!["plug", "--flag", "run", "--output", "x", "r"]
        );
    }

    #[test]
    fn external_command_preserves_separator_and_empty_tokens() {
        let (_, _, tokens) = external_ok(&["tool", "plug", "--", ""]);
        assert_eq!(tokens, vec!["plug", "--", ""]);

        let (_, _, tokens) = external_ok(&["tool", "plug", "", "--", ""]);
        assert_eq!(tokens, vec!["plug", "", "--", ""]);
    }

    #[test]
    fn external_help_like_token_is_data() {
        let (_, _, tokens) = external_ok(&["tool", "plug", "--help", "--version"]);
        assert_eq!(tokens, vec!["plug", "--help", "--version"]);
    }

    #[test]
    fn external_name_may_look_like_a_flag_after_root_escape() {
        let (_, _, tokens) = external_ok(&["tool", "--", "--flag", "x", "--"]);
        assert_eq!(tokens, vec!["--flag", "x", "--"]);
    }

    #[test]
    fn external_name_can_be_empty() {
        let (_, _, tokens) = external_ok(&["tool", ""]);
        assert_eq!(tokens, vec![""]);

        let (_, _, tokens) = external_ok(&["tool", "--", "", "rest"]);
        assert_eq!(tokens, vec!["", "rest"]);
    }

    #[test]
    fn root_escape_dispatches_declared_name_to_external() {
        let (_, _, tokens) = external_ok(&["tool", "--", "run", "--output"]);
        assert_eq!(tokens, vec!["run", "--output"]);

        let (_, _, tokens) = external_ok(&["tool", "--", "r"]);
        assert_eq!(tokens, vec!["r"]);
    }

    #[test]
    fn global_options_parse_before_external_command() {
        let (config, define, tokens) = external_ok(&[
            "tool",
            "--config",
            "custom.toml",
            "--define",
            "a=1",
            "--define",
            "b=2",
            "plug",
            "arg",
        ]);
        assert_eq!(config, "custom.toml");
        assert_eq!(define, vec!["a=1", "b=2"]);
        assert_eq!(tokens, vec!["plug", "arg"]);
    }

    #[test]
    fn global_options_after_external_name_are_captured() {
        let (config, define, tokens) = external_ok(&[
            "tool",
            "--config",
            "custom.toml",
            "plug",
            "--config",
            "stolen.toml",
            "--define",
            "c=3",
        ]);
        assert_eq!(config, "custom.toml");
        assert_eq!(define, Vec::<String>::new());
        assert_eq!(
            tokens,
            vec!["plug", "--config", "stolen.toml", "--define", "c=3"]
        );
    }

    #[test]
    fn globals_beside_run_still_reach_run() {
        let (config, define, run) = run_ok(&[
            "tool",
            "run",
            "--config",
            "x.toml",
            "--output",
            "o",
            "--define",
            "k=v",
        ]);
        assert_eq!(config, "x.toml");
        assert_eq!(define, vec!["k=v"]);
        assert_eq!(run.output, "o");
    }

    #[test]
    fn global_options_accept_equals_form_before_external_command() {
        let (config, define, tokens) = external_ok(&[
            "tool",
            "--config=custom.toml",
            "--define=a=1",
            "plug",
        ]);
        assert_eq!(config, "custom.toml");
        assert_eq!(define, vec!["a=1"]);
        assert_eq!(tokens, vec!["plug"]);
    }

    #[test]
    fn root_escape_can_name_double_dash_itself() {
        // The first `--` ends option parsing; the second `--` is the command
        // name and the third is an ordinary argument.
        let (_, _, tokens) = external_ok(&["tool", "--", "--", "--"]);
        assert_eq!(tokens, vec!["--", "--"]);
    }

    #[test]
    fn unknown_option_before_external_name_fails_on_that_token() {
        for argv in [
            &["tool", "--bogus", "plug"][..],
            &["tool", "--config", "c.toml", "--bogus", "plug"][..],
        ] {
            let err = parse(argv).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::UnknownArgument, "{argv:?}");
            assert!(err.to_string().contains("--bogus"), "{argv:?}");
        }

        let err = parse(&["tool", "-x", "plug"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        assert!(err.to_string().contains("-x"));
    }

    #[test]
    fn missing_option_value_before_external_name_fails() {
        for argv in [
            &["tool", "--config"][..],
            &["tool", "--define"][..],
            &["tool", "--config", "c.toml", "--define"][..],
        ] {
            let err = parse(argv).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidValue, "{argv:?}");
            let msg = err.to_string();
            let needle = if argv[argv.len() - 1] == "--config" {
                "--config"
            } else {
                "--define"
            };
            assert!(msg.contains(needle), "{argv:?}: {msg}");
        }
    }

    #[test]
    fn invalid_define_before_external_name_fails_on_value() {
        for argv in [
            &["tool", "--define", "nokey", "plug"][..],
            &["tool", "--define=nokey"][..],
        ] {
            let err = parse(argv).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::ValueValidation, "{argv:?}");
            assert!(err.to_string().contains("nokey"), "{argv:?}");
        }
    }

    #[test]
    fn invalid_define_after_external_name_is_captured_as_data() {
        let (_, define, tokens) = external_ok(&["tool", "plug", "--define", "nokey"]);
        assert_eq!(define, Vec::<String>::new());
        assert_eq!(tokens, vec!["plug", "--define", "nokey"]);
    }

    #[test]
    fn root_help_succeeds_without_subcommand() {
        let err = parse(&["tool", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        let help = err.to_string();
        for needle in ["--config", "--define", "builtin.toml", "key=value", "run"] {
            assert!(help.contains(needle), "root help missing {needle}:\n{help}");
        }
        // Subcommand-specific options must not leak into root help.
        assert!(!help.contains("--output"), "{help}");
        assert!(!help.trim_end().ends_with("--help"), "{help}");
    }

    #[test]
    fn run_help_succeeds_without_output() {
        let err = parse(&["tool", "run", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        let help = err.to_string();
        assert!(help.contains("--output"), "{help}");
        assert!(help.contains("tool run"), "{help}");
        // DisplayHelp proves `--help` selected help instead of being captured.

        let err = parse(&["tool", "r", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    }

    #[test]
    fn help_after_failure_is_clean() {
        let err = parse(&["tool", "--bogus-flag"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        assert!(err.to_string().contains("--bogus-flag"));

        let help = parse(&["tool", "--help"]).unwrap_err();
        assert_eq!(help.kind(), ErrorKind::DisplayHelp);
        let help = help.to_string();
        assert!(!help.contains("--bogus-flag"), "{help}");
        assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
    }

    #[test]
    fn success_failure_success_calls_are_isolated() {
        // 1. external success
        let (config, define, tokens) = external_ok(&["tool", "plug", "--define", "x=1"]);
        assert_eq!(config, "builtin.toml");
        assert_eq!(define, Vec::<String>::new());
        assert_eq!(tokens, vec!["plug", "--define", "x=1"]);

        // 2. invalid root input fails
        let err = parse(&["tool", "--config"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidValue);

        // 3. run success is unaffected by either previous call
        let (config, define, run) = run_ok(&["tool", "run", "--output", "o"]);
        assert_eq!(config, "builtin.toml");
        assert_eq!(define, Vec::<String>::new());
        assert_eq!(run.output, "o");
        assert_eq!(run.trailing, Vec::<String>::new());
    }
}
