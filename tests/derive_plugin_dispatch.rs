//! Standalone, independently executable regression target for derive-based
//! dispatch between a declared subcommand (`run`, aliased `r`) and
//! plugin-provided external subcommands.
//!
//! Unlike the private modules under `tests/derive/`, this file compiles into
//! its own test binary and defines its own `Parser` types.  Every observation
//! goes through the public `clap::Parser::try_parse_from` with argv0 included:
//! success structures, error kinds/text, and help output are all asserted on
//! directly rather than relying on a test process exit code.
#![cfg(feature = "derive")]

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::error::ErrorKind;

const DEFAULT_CONFIG: &str = "builtin.toml";

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
    #[arg(long, global = true, default_value = DEFAULT_CONFIG, value_name = "path")]
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

#[derive(Args, Clone, Debug, PartialEq)]
#[command(override_usage = "tool run [OPTIONS] --output <path> [-- <TRAILING>...]")]
pub struct RunArgs {
    #[arg(long, value_name = "path", required = true)]
    pub output: String,
    #[arg(last = true)]
    pub trailing: Vec<String>,
}

fn parse(argv: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(argv)
}

fn expect_external(argv: &[&str]) -> (String, Vec<String>, Vec<String>) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::External(tokens) = cli.command else {
        panic!(
            "expected external subcommand for {argv:?}, got {:?}",
            cli.command
        );
    };
    assert!(
        !tokens.is_empty(),
        "external capture must always retain the command name: {argv:?}"
    );
    (cli.config, cli.define, tokens)
}

fn expect_run(argv: &[&str]) -> (String, Vec<String>, RunArgs) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::Run(run) = cli.command else {
        panic!("expected `run` parse for {argv:?}, got {:?}", cli.command);
    };
    (cli.config, cli.define, run)
}

fn expect_err(argv: &[&str]) -> clap::Error {
    parse(argv)
        .err()
        .unwrap_or_else(|| panic!("expected failure for {argv:?}"))
}

#[test]
fn root_defaults_apply_when_no_globals_are_given() {
    let (config, define, tokens) = expect_external(&["tool", "plug", "arg"]);
    assert_eq!(config, DEFAULT_CONFIG);
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "arg"]);
}

#[test]
fn explicit_config_overrides_default_before_external_name() {
    let (config, _, tokens) = expect_external(&["tool", "--config", "custom.toml", "plug"]);
    assert_eq!(config, "custom.toml");
    assert_eq!(tokens, vec!["plug"]);

    let (config, _, _) = expect_external(&["tool", "--config=equals.toml", "plug"]);
    assert_eq!(config, "equals.toml");
}

#[test]
fn repeatable_defines_keep_definition_order() {
    let (config, define, _) = expect_external(&[
        "tool",
        "--define",
        "b=2",
        "--define=a=1",
        "--config",
        "c.toml",
        "--define",
        "c=3",
        "plug",
    ]);
    assert_eq!(config, "c.toml");
    assert_eq!(define, vec!["b=2", "a=1", "c=3"]);
}

#[test]
fn external_name_and_every_following_token_are_saved_verbatim() {
    // Tokens that look like options, declared subcommands, a second
    // separator, or an empty string are all data once dispatch happened.
    let (_, _, tokens) = expect_external(&[
        "tool", "plug", "--flag", "run", "r", "--output", "x", "--", "",
    ]);
    assert_eq!(
        tokens,
        vec!["plug", "--flag", "run", "r", "--output", "x", "--", ""]
    );

    let (_, _, tokens) = expect_external(&["tool", "plug", "", "--help", "--version"]);
    assert_eq!(tokens, vec!["plug", "", "--help", "--version"]);
}

#[test]
fn root_globals_after_external_name_are_captured_not_consumed() {
    let (config, define, tokens) = expect_external(&[
        "tool",
        "--config",
        "first.toml",
        "plug",
        "--config",
        "stolen.toml",
        "--define",
        "k=v",
    ]);
    assert_eq!(config, "first.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        tokens,
        vec!["plug", "--config", "stolen.toml", "--define", "k=v"]
    );
}

#[test]
fn a_name_prefix_of_run_is_an_external_command() {
    let (_, _, tokens) = expect_external(&["tool", "ru", "--output", "x"]);
    assert_eq!(tokens, vec!["ru", "--output", "x"]);
}

#[test]
fn root_double_dash_makes_next_token_the_external_command_name() {
    let (_, _, tokens) = expect_external(&["tool", "--", "plug", "--flag", "x"]);
    assert_eq!(tokens, vec!["plug", "--flag", "x"]);

    // A flag-looking token is a valid external command name after escape.
    let (_, _, tokens) = expect_external(&["tool", "--", "--flag", "x", "--"]);
    assert_eq!(tokens, vec!["--flag", "x", "--"]);

    // The declared name reached via escape is dispatched externally.
    let (_, _, tokens) = expect_external(&["tool", "--", "run", "--output"]);
    assert_eq!(tokens, vec!["run", "--output"]);
    let (_, _, tokens) = expect_external(&["tool", "--", "r"]);
    assert_eq!(tokens, vec!["r"]);

    // Empty name, and the first `--` ends option parsing so the second
    // `--` is the command name and the third is an ordinary argument.
    let (_, _, tokens) = expect_external(&["tool", "--", "", "rest"]);
    assert_eq!(tokens, vec!["", "rest"]);
    let (_, _, tokens) = expect_external(&["tool", "--", "--", "--"]);
    assert_eq!(tokens, vec!["--", "--"]);
}

#[test]
fn run_and_alias_r_share_parsing_and_output_validation() {
    for argv in [
        &["tool", "run", "--output", "out.txt"][..],
        &["tool", "r", "--output", "out.txt"][..],
    ] {
        let (config, define, run) = expect_run(argv);
        assert_eq!(config, DEFAULT_CONFIG);
        assert_eq!(define, Vec::<String>::new());
        assert_eq!(run.output, "out.txt");
        assert_eq!(run.trailing, Vec::<String>::new());
    }

    for argv in [
        &["tool", "run", "--output", "o", "--", "a", "--flag"][..],
        &["tool", "r", "--output", "o", "--", "a", "--flag"][..],
    ] {
        let (_, _, run) = expect_run(argv);
        assert_eq!(run.output, "o");
        assert_eq!(run.trailing, vec!["a", "--flag"]);
    }
}

#[test]
fn run_separator_only_produces_trailing_args_never_external_dispatch() {
    for name in ["run", "r"] {
        let (config, define, run) = expect_run(&[
            "tool",
            name,
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
        // Tokens after run's separator must not be reinterpreted at root.
        assert_eq!(config, DEFAULT_CONFIG);
        assert_eq!(define, Vec::<String>::new());
    }
}

#[test]
fn globals_beside_run_reach_run_for_both_names() {
    for name in ["run", "r"] {
        let (config, define, run) = expect_run(&[
            "tool", name, "--config", "x.toml", "--output", "o", "--define", "k=v",
        ]);
        assert_eq!(config, "x.toml");
        assert_eq!(define, vec!["k=v"]);
        assert_eq!(run.output, "o");
    }
}

#[test]
fn missing_output_fails_for_both_names_with_specific_context() {
    for argv in [&["tool", "run"][..], &["tool", "r"][..]] {
        let err = expect_err(argv);
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument, "{argv:?}");
        #[cfg(feature = "error-context")]
        {
            let msg = err.to_string();
            assert!(msg.contains("--output"), "{argv:?}: {msg}");
            #[cfg(feature = "usage")]
            assert!(msg.contains("tool run"), "{argv:?}: {msg}");
        }
    }
}

#[test]
fn duplicate_output_is_rejected_for_both_names() {
    for argv in [
        &["tool", "run", "--output", "a", "--output", "b"][..],
        &["tool", "r", "--output", "a", "--output", "b"][..],
    ] {
        let err = expect_err(argv);
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict, "{argv:?}");
        #[cfg(feature = "error-context")]
        {
            let msg = err.to_string();
            assert!(msg.contains("--output"), "{argv:?}: {msg}");
            #[cfg(feature = "usage")]
            assert!(msg.contains("tool run"), "{argv:?}: {msg}");
        }
    }
}

#[test]
fn unknown_root_option_fails_on_that_token_at_root() {
    for argv in [
        &["tool", "--bogus", "plug"][..],
        &["tool", "--config", "c.toml", "--bogus", "plug"][..],
    ] {
        let err = expect_err(argv);
        assert_eq!(err.kind(), ErrorKind::UnknownArgument, "{argv:?}");
        #[cfg(feature = "error-context")]
        {
            let msg = err.to_string();
            assert!(msg.contains("--bogus"), "{argv:?}: {msg}");
            #[cfg(feature = "usage")]
            assert!(msg.contains("tool [OPTIONS] <COMMAND>"), "{argv:?}: {msg}");
        }
    }

    let err = expect_err(&["tool", "-x", "plug"]);
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    #[cfg(feature = "error-context")]
    assert!(err.to_string().contains("-x"));
}

#[test]
fn unknown_option_after_run_fails_with_run_context() {
    for argv in [
        &["tool", "run", "--output", "o", "--bogus"][..],
        &["tool", "r", "--output", "o", "--bogus"][..],
    ] {
        let err = expect_err(argv);
        assert_eq!(err.kind(), ErrorKind::UnknownArgument, "{argv:?}");
        #[cfg(feature = "error-context")]
        {
            let msg = err.to_string();
            assert!(msg.contains("--bogus"), "{argv:?}: {msg}");
            #[cfg(feature = "usage")]
            assert!(msg.contains("tool run"), "{argv:?}: {msg}");
        }
    }
}

#[test]
fn positional_before_run_separator_is_an_error_not_a_half_match() {
    for argv in [
        &["tool", "run", "--output", "o", "extra"][..],
        &["tool", "r", "extra"][..],
    ] {
        let err = expect_err(argv);
        assert_eq!(err.kind(), ErrorKind::UnknownArgument, "{argv:?}");
        #[cfg(feature = "error-context")]
        assert!(err.to_string().contains("extra"), "{argv:?}");
    }
}

#[test]
fn missing_option_values_fail_on_the_offending_option() {
    let err = expect_err(&["tool", "--config"]);
    assert_eq!(err.kind(), ErrorKind::InvalidValue);
    #[cfg(feature = "error-context")]
    assert!(err.to_string().contains("--config"));

    let err = expect_err(&["tool", "--define"]);
    assert_eq!(err.kind(), ErrorKind::InvalidValue);
    #[cfg(feature = "error-context")]
    assert!(err.to_string().contains("--define"));

    let err = expect_err(&["tool", "--config", "c.toml", "--define"]);
    assert_eq!(err.kind(), ErrorKind::InvalidValue);
    #[cfg(feature = "error-context")]
    assert!(err.to_string().contains("--define"));

    for argv in [
        &["tool", "run", "--output"][..],
        &["tool", "r", "--output"][..],
    ] {
        let err = expect_err(argv);
        assert_eq!(err.kind(), ErrorKind::InvalidValue, "{argv:?}");
        #[cfg(feature = "error-context")]
        assert!(err.to_string().contains("--output"), "{argv:?}");
    }
}

#[test]
fn invalid_define_fails_on_value_before_external_name() {
    for argv in [
        &["tool", "--define", "nokey", "plug"][..],
        &["tool", "--define=nokey"][..],
    ] {
        let err = expect_err(argv);
        assert_eq!(err.kind(), ErrorKind::ValueValidation, "{argv:?}");
        #[cfg(feature = "error-context")]
        {
            let msg = err.to_string();
            assert!(msg.contains("nokey"), "{argv:?}: {msg}");
            assert!(msg.contains("key=value"), "{argv:?}: {msg}");
        }
    }
}

#[test]
fn invalid_define_after_external_name_is_data_not_validated() {
    let (config, define, tokens) = expect_external(&["tool", "plug", "--define", "nokey"]);
    assert_eq!(config, DEFAULT_CONFIG);
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "--define", "nokey"]);
}

#[cfg(feature = "help")]
#[test]
fn root_help_succeeds_without_a_subcommand_and_stays_at_root() {
    let err = expect_err(&["tool", "--help"]);
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let help = err.to_string();
    for needle in [
        "--config",
        "--define",
        DEFAULT_CONFIG,
        "key=value",
        "run",
        "Run a job",
    ] {
        assert!(help.contains(needle), "root help missing {needle}:\n{help}");
    }
    // Subcommand-specific options and error text must not leak into help.
    assert!(!help.contains("--output"), "{help}");
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[cfg(feature = "help")]
#[test]
fn run_help_succeeds_without_required_output_for_both_names() {
    let via_run = expect_err(&["tool", "run", "--help"]);
    assert_eq!(via_run.kind(), ErrorKind::DisplayHelp);
    let via_run = via_run.to_string();

    let via_alias = expect_err(&["tool", "r", "--help"]);
    assert_eq!(via_alias.kind(), ErrorKind::DisplayHelp);
    let via_alias = via_alias.to_string();

    for help in [&via_run, &via_alias] {
        for needle in ["--output", "Run a job", "--config", "--define"] {
            assert!(help.contains(needle), "run help missing {needle}:\n{help}");
        }
        assert!(help.contains("tool run"), "{help}");
    }
    assert_eq!(via_run, via_alias);
}

#[cfg(feature = "help")]
#[test]
fn help_after_a_failure_contains_no_trace_of_the_error() {
    let err = expect_err(&["tool", "--bogus-flag"]);
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    #[cfg(feature = "error-context")]
    assert!(err.to_string().contains("--bogus-flag"));

    let err = expect_err(&["tool", "run", "--output"]);
    assert_eq!(err.kind(), ErrorKind::InvalidValue);

    for argv in [&["tool", "--help"][..], &["tool", "run", "--help"][..]] {
        let help = expect_err(argv);
        assert_eq!(help.kind(), ErrorKind::DisplayHelp, "{argv:?}");
        let help = help.to_string();
        assert!(!help.contains("--bogus-flag"), "{argv:?}:\n{help}");
        assert!(
            !help.to_ascii_lowercase().contains("error"),
            "{argv:?}:\n{help}"
        );
    }
}

/// Fixed-order sequence proving calls cannot distort each other through test
/// adapters or shared state: external success, invalid root input, run
/// success, then help requests are checked item by item in a single test.
#[cfg(feature = "help")]
#[test]
fn fixed_order_success_failure_success_help_sequence_is_isolated() {
    // 1. External success: globals before the name parse; the declared
    //    subcommand name, a second separator, and an empty token after it
    //    are captured verbatim.
    let (config, define, tokens) = expect_external(&[
        "tool",
        "--config",
        "custom.toml",
        "--define",
        "a=1",
        "plug",
        "--flag",
        "run",
        "--",
        "",
    ]);
    assert_eq!(config, "custom.toml");
    assert_eq!(define, vec!["a=1"]);
    assert_eq!(tokens, vec!["plug", "--flag", "run", "--", ""]);

    // 2. Invalid root input fails hard on the trigger token at root and
    //    yields no half-finished match to continue reading.
    let err = expect_err(&["tool", "--bogus"]);
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    #[cfg(feature = "error-context")]
    {
        let msg = err.to_string();
        assert!(msg.contains("--bogus"), "{msg}");
        #[cfg(feature = "usage")]
        assert!(msg.contains("tool [OPTIONS] <COMMAND>"), "{msg}");
    }

    // 3. `run` success through the alias: it must see neither the globals
    //    from step 1 nor the failure from step 2.
    let (config, define, run) =
        expect_run(&["tool", "r", "--output", "o", "--", "x", "--config", "z"]);
    assert_eq!(config, DEFAULT_CONFIG);
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(run.output, "o");
    assert_eq!(run.trailing, vec!["x", "--config", "z"]);

    // 4a. Run help succeeds despite the missing `--output` in step 2's
    //     sibling parse, carries run-specific context, and is unpolluted.
    let help = expect_err(&["tool", "run", "--help"]);
    assert_eq!(help.kind(), ErrorKind::DisplayHelp);
    let help = help.to_string();
    for needle in ["--output", "tool run", "Run a job", DEFAULT_CONFIG] {
        assert!(help.contains(needle), "run help missing {needle}:\n{help}");
    }
    assert!(!help.contains("--bogus"), "{help}");
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");

    // 4b. Root help likewise reflects only root state.
    let help = expect_err(&["tool", "--help"]);
    assert_eq!(help.kind(), ErrorKind::DisplayHelp);
    let help = help.to_string();
    for needle in ["--config", "--define", DEFAULT_CONFIG, "run"] {
        assert!(help.contains(needle), "root help missing {needle}:\n{help}");
    }
    assert!(!help.contains("--output"), "{help}");
    assert!(!help.contains("--bogus"), "{help}");
}
