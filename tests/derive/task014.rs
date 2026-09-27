//! Derived CLI with a declared `run` subcommand (alias `r`) and an
//! `external_subcommand` fallback that hands unknown first-level commands to
//! plugins. All behavior is observed through `Cli::try_parse_from` with token
//! sequences that include argv0.
#![allow(unreachable_pub)] // types stay observable to sibling test modules

use clap::Parser;
use clap::Subcommand;

/// `--define` only accepts `key=value` with a non-empty key.
fn parse_define(raw: &str) -> Result<String, String> {
    match raw.split_once('=') {
        Some((key, _)) if !key.is_empty() => Ok(raw.to_owned()),
        _ => Err(format!("invalid define `{raw}`: expected key=value")),
    }
}

#[derive(Parser, Debug, PartialEq)]
#[command(
    name = "tool",
    about = "Root command with global options and plugin fallback",
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
    #[command(alias = "r", about = "Run a task")]
    Run(RunArgs),
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(clap::Args, Debug, PartialEq)]
#[command(override_usage = "tool run [OPTIONS] --output <path> [-- <TRAILING>...]")]
pub struct RunArgs {
    #[arg(long, value_name = "path")]
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

    fn external(argv: &[&str]) -> Vec<String> {
        let cli = parse(argv).unwrap_or_else(|e| panic!("expected success for {argv:?}: {e}"));
        let Commands::External(tokens) = cli.command else {
            panic!("expected external capture for {argv:?}");
        };
        tokens
    }

    fn run_ok(argv: &[&str]) -> (Cli, RunArgs) {
        let cli = parse(argv).unwrap_or_else(|e| panic!("expected success for {argv:?}: {e}"));
        let Commands::Run(run) = &cli.command else {
            panic!("expected `run` parse for {argv:?}");
        };
        let run = RunArgs {
            output: run.output.clone(),
            trailing: run.trailing.clone(),
        };
        (cli, run)
    }

    #[test]
    fn declared_run_and_alias_route_to_run() {
        let (cli, run) = run_ok(&["tool", "run", "--output", "out.txt"]);
        assert_eq!(cli.config, "builtin.toml");
        assert_eq!(cli.define, Vec::<String>::new());
        assert_eq!(run.output, "out.txt");
        assert_eq!(run.trailing, Vec::<String>::new());

        let (_, run) = run_ok(&["tool", "r", "--output", "a"]);
        assert_eq!(run.output, "a");

        // Globals stay usable around the alias.
        let (cli, _) = run_ok(&["tool", "--config", "c.toml", "r", "--output", "a"]);
        assert_eq!(cli.config, "c.toml");
    }

    #[test]
    fn run_validates_output_inside_the_subcommand() {
        let err = parse(&["tool", "run"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
        let msg = err.to_string();
        assert!(msg.contains("--output"), "{msg}");
        assert!(msg.contains("tool run"), "{msg}");

        let err = parse(&["tool", "r"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
        assert!(err.to_string().contains("--output"));

        let err = parse(&["tool", "run", "--output"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
        assert!(err.to_string().contains("--output"));

        // Tokens before the separator are still run arguments, not a plugin.
        let err = parse(&["tool", "run", "--output", "o", "stray"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        assert!(err.to_string().contains("stray"));
    }

    #[test]
    fn unknown_first_level_token_is_external_command() {
        assert_eq!(external(&["tool", "plugin"]), vec!["plugin"]);
        // Near-misses of the declared name are plugins, not fuzzy matches.
        assert_eq!(external(&["tool", "ru"]), vec!["ru"]);
        assert_eq!(external(&["tool", "runx"]), vec!["runx"]);
    }

    #[test]
    fn external_tokens_are_preserved_verbatim() {
        assert_eq!(
            external(&["tool", "plugin", "--flag", "--output", "x"]),
            vec!["plugin", "--flag", "--output", "x"]
        );
        // Tokens that look like subcommands stay plugin arguments.
        assert_eq!(
            external(&["tool", "plugin", "run", "r", "help"]),
            vec!["plugin", "run", "r", "help"]
        );
        // Standalone separators and empty strings survive untouched.
        assert_eq!(
            external(&["tool", "plugin", "--", "--", ""]),
            vec!["plugin", "--", "--", ""]
        );
        // The empty string is itself a valid external command name.
        assert_eq!(external(&["tool", ""]), vec![""]);
    }

    #[test]
    fn globals_before_external_name_are_parsed_in_order() {
        let cli = parse(&[
            "tool",
            "--config",
            "custom.toml",
            "--define",
            "b=2",
            "--define",
            "a=1",
            "plugin",
        ])
        .unwrap();
        assert_eq!(cli.config, "custom.toml");
        assert_eq!(cli.define, vec!["b=2", "a=1"]);
        let Commands::External(tokens) = cli.command else {
            panic!("expected external");
        };
        assert_eq!(tokens, vec!["plugin"]);

        // `--config=value` form overrides the default as well.
        let cli = parse(&["tool", "--config=c.toml", "plugin"]).unwrap();
        assert_eq!(cli.config, "c.toml");
    }

    #[test]
    fn globals_after_external_name_belong_to_the_plugin() {
        let cli = parse(&[
            "tool",
            "--config",
            "c.toml",
            "plugin",
            "--config",
            "x",
            "--define",
            "k=v",
        ])
        .unwrap();
        // The root-level config is not stolen back out of plugin arguments.
        assert_eq!(cli.config, "c.toml");
        assert_eq!(cli.define, Vec::<String>::new());
        let Commands::External(tokens) = cli.command else {
            panic!("expected external");
        };
        assert_eq!(
            tokens,
            vec!["plugin", "--config", "x", "--define", "k=v"]
        );
    }

    #[test]
    fn root_failures_before_the_plugin_name_are_reported() {
        let err = parse(&["tool", "--bogus", "plugin"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        assert!(err.to_string().contains("--bogus"), "{}", err);

        let err = parse(&["tool", "--config"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
        assert!(err.to_string().contains("--config"), "{}", err);

        let err = parse(&["tool", "--define"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
        assert!(err.to_string().contains("--define"), "{}", err);

        // The malformed value itself is the most specific trigger token.
        let err = parse(&["tool", "--define", "bad", "plugin"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ValueValidation);
        let msg = err.to_string();
        assert!(msg.contains("bad"), "{msg}");
        assert!(msg.contains("key=value"), "{msg}");

        let err = parse(&["tool", "--define", "=v", "plugin"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ValueValidation);
        assert!(err.to_string().contains("=v"));

        // A valid define still parses, including an empty value after `=`.
        let cli = parse(&["tool", "--define", "k=", "plugin"]).unwrap();
        assert_eq!(cli.define, vec!["k="]);
        let cli = parse(&["tool", "--define=a=b", "plugin"]).unwrap();
        assert_eq!(cli.define, vec!["a=b"]);
    }

    #[test]
    fn root_double_dash_makes_next_token_the_plugin_name() {
        assert_eq!(
            external(&["tool", "--", "plugin", "--flag"]),
            vec!["plugin", "--flag"]
        );
        // After `--`, even flag-looking tokens are command names verbatim.
        assert_eq!(external(&["tool", "--", "--flag"]), vec!["--flag"]);
        assert_eq!(external(&["tool", "--", ""]), vec![""]);
        assert_eq!(
            external(&["tool", "--", "run", "--output", "o"]),
            vec!["run", "--output", "o"]
        );
        // `--` with no token after it cannot name a plugin.
        let err = parse(&["tool", "--"]).unwrap_err();
        assert_eq!(
            err.kind(),
            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
        );
    }

    #[test]
    fn run_double_dash_keeps_run_trailing_semantics() {
        let (cli, run) = run_ok(&[
            "tool",
            "run",
            "--output",
            "o",
            "--",
            "--config",
            "run",
            "r",
            "--",
            "",
        ]);
        assert_eq!(run.output, "o");
        assert_eq!(
            run.trailing,
            vec!["--config", "run", "r", "--", ""]
        );
        // Nothing after the separator leaks back into root globals.
        assert_eq!(cli.config, "builtin.toml");
    }

    #[test]
    fn help_succeeds_even_without_output() {
        let err = parse(&["tool", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        let root = err.to_string();
        assert!(root.contains("Usage:"), "{root}");
        for needle in ["run", "--config", "--define", "builtin.toml"] {
            assert!(root.contains(needle), "root help missing {needle}:\n{root}");
        }
        // Help output carries no leftover failure text.
        assert!(!root.contains("error:"), "{root}");

        let err = parse(&["tool", "run", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        let run = err.to_string();
        assert!(run.contains("Usage:"), "{run}");
        for needle in ["--output", "tool run", "TRAILING"] {
            assert!(run.contains(needle), "run help missing {needle}:\n{run}");
        }
        assert!(!run.contains("error:"), "{run}");

        let err = parse(&["tool", "r", "--help"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);

        // `--help` short-circuits before a following token can be a plugin...
        let err = parse(&["tool", "--help", "plugin"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        // ...while `--help` after a plugin name belongs to the plugin.
        assert_eq!(
            external(&["tool", "plugin", "--help"]),
            vec!["plugin", "--help"]
        );
    }

    #[test]
    fn repeated_parses_are_isolated() {
        // External success, root failure, run success, repeated three times.
        for round in 0..3 {
            let cli = parse(&["tool", "plugin", "--config", "x", "--bogus"]).unwrap();
            // No globals are stolen out from after the plugin name.
            assert_eq!(cli.config, "builtin.toml", "round {round}");
            assert_eq!(
                cli.command,
                Commands::External(vec![
                    "plugin".into(),
                    "--config".into(),
                    "x".into(),
                    "--bogus".into(),
                ]),
                "round {round}"
            );

            let err = parse(&["tool", "--define", "bad", "plugin"]).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::ValueValidation, "round {round}");

            let (cli, run) = run_ok(&[
                "tool",
                "--define",
                "k=v",
                "run",
                "--output",
                "o",
                "--",
                "plugin",
            ]);
            assert_eq!(cli.config, "builtin.toml", "round {round}");
            assert_eq!(cli.define, vec!["k=v"], "round {round}");
            assert_eq!(run.output, "o", "round {round}");
            assert_eq!(run.trailing, vec!["plugin"], "round {round}");
        }
    }
}
