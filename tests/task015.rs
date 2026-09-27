//! Standalone, executable regression target for external-plugin dispatch.
//!
//! Unlike the tests under `tests/derive/`, this file is a self-contained test
//! target that does not depend on any private test module from a candidate
//! branch. It defines its own derived CLI and observes every result solely
//! through the public `clap::Parser` API via `Cli::try_parse_from` (argv0
//! included): successful structures, typed errors, and rendered help text.
//!
//! Scenario under test:
//! * root command `tool` with a declared `run` subcommand (alias `r`) and an
//!   external-subcommand catch-all for plugins;
//! * root-level default config path and repeatable `--define` items that only
//!   parse while the root command is active;
//! * every token at or after the external command name is captured verbatim,
//!   including option-like tokens, declared subcommand names, a second `--`,
//!   and empty strings;
//! * a root-level `--` forces the next token to be the external command name,
//!   while `--` after `run` only produces `run` trailing arguments;
//! * validation failures name the triggering token and the most specific
//!   command and never leave a partially populated match readable;
//! * root and `run` help succeed while required values are missing;
//! * a fixed sequence of success, failure, success, and help parses is
//!   mutually isolated.
#![cfg(feature = "derive")]
#![cfg(feature = "help")]
#![cfg(feature = "usage")]

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::error::ErrorKind;

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
struct Cli {
    #[arg(
        long,
        global = true,
        default_value = "builtin.toml",
        value_name = "path"
    )]
    config: String,
    #[arg(long, global = true, value_name = "key=value", value_parser = parse_define)]
    define: Vec<String>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, PartialEq)]
enum Commands {
    #[command(about = "Run a job", alias = "r")]
    Run(RunArgs),
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args, Debug, PartialEq)]
#[command(override_usage = "tool run [OPTIONS] --output <path> [-- <TRAILING>...]")]
struct RunArgs {
    #[arg(long, value_name = "path", required = true)]
    output: String,
    #[arg(last = true)]
    trailing: Vec<String>,
}

fn parse(argv: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(argv)
}

fn external_ok(argv: &[&str]) -> (String, Vec<String>, Vec<String>) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::External(tokens) = cli.command else {
        panic!(
            "expected external subcommand for {argv:?}, got {:?}",
            cli.command
        );
    };
    assert!(
        !tokens.is_empty(),
        "external capture must always include the command name: {argv:?}"
    );
    (cli.config, cli.define, tokens)
}

fn run_ok(argv: &[&str]) -> (String, Vec<String>, RunArgs) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::Run(run) = cli.command else {
        panic!("expected `run` parse for {argv:?}, got {:?}", cli.command);
    };
    (cli.config, cli.define, run)
}

/// Render an error while dropping the optional backtrace appended by clap's
/// `debug` feature. The backtrace captures host paths and frame line numbers
/// and is not part of the user-facing help or error output under test.
fn rendered(err: &clap::Error) -> String {
    let text = err.to_string();
    match text.split_once("\nBacktrace:") {
        Some((head, _)) => head.trim_end().to_owned(),
        None => text,
    }
}

fn fail_kind(argv: &[&str]) -> (ErrorKind, String) {
    match parse(argv) {
        Ok(cli) => panic!("expected failure for {argv:?}, got {cli:?}"),
        Err(err) => {
            let kind = err.kind();
            (kind, rendered(&err))
        }
    }
}

// ---- successful external dispatch --------------------------------------

#[test]
fn external_dispatch_uses_root_defaults() {
    let (config, define, tokens) = external_ok(&["tool", "plug", "arg"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "arg"]);
}

#[test]
fn explicit_config_overrides_default_and_defines_keep_order() {
    let (config, define, tokens) = external_ok(&[
        "tool",
        "--define",
        "a=1",
        "--config",
        "custom.toml",
        "--define",
        "b=2",
        "--define=c=3",
        "plug",
    ]);
    assert_eq!(config, "custom.toml");
    assert_eq!(define, vec!["a=1", "b=2", "c=3"]);
    assert_eq!(tokens, vec!["plug"]);
}

#[test]
fn external_captures_name_and_every_following_token_verbatim() {
    let (_, _, tokens) = external_ok(&[
        "tool", "plug", "--flag", "run", "r", "--", "", "--config", "x",
    ]);
    assert_eq!(
        tokens,
        vec!["plug", "--flag", "run", "r", "--", "", "--config", "x"]
    );
}

#[test]
fn help_and_version_like_tokens_after_name_are_data() {
    let (_, _, tokens) = external_ok(&["tool", "plug", "--help", "--version", "-h"]);
    assert_eq!(tokens, vec!["plug", "--help", "--version", "-h"]);
}

#[test]
fn root_options_after_external_name_are_captured_not_stolen() {
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
fn prefix_of_declared_command_is_external() {
    let (_, _, tokens) = external_ok(&["tool", "ru", "--output", "x"]);
    assert_eq!(tokens, vec!["ru", "--output", "x"]);
}

// ---- root-level `--` ------------------------------------------------------

#[test]
fn root_double_dash_uses_next_token_as_external_name() {
    // The token right after the root separator is the command name, even when
    // it looks like a flag; the second separator and empty string stay in order.
    let (config, define, tokens) =
        external_ok(&["tool", "--config", "c.toml", "--", "--flag", "x", "--", ""]);
    assert_eq!(config, "c.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["--flag", "x", "--", ""]);
}

#[test]
fn root_double_dash_can_name_double_dash_itself() {
    let (_, _, tokens) = external_ok(&["tool", "--", "--", "--"]);
    assert_eq!(tokens, vec!["--", "--"]);
}

#[test]
fn root_double_dash_can_name_empty_command() {
    let (_, _, tokens) = external_ok(&["tool", "--", "", "rest"]);
    assert_eq!(tokens, vec!["", "rest"]);

    let (_, _, tokens) = external_ok(&["tool", ""]);
    assert_eq!(tokens, vec![""]);
}

#[test]
fn root_double_dash_routes_declared_names_to_external_capture() {
    let (_, _, tokens) = external_ok(&["tool", "--", "run", "--output"]);
    assert_eq!(tokens, vec!["run", "--output"]);

    let (_, _, tokens) = external_ok(&["tool", "--", "r"]);
    assert_eq!(tokens, vec!["r"]);
}

// ---- `run` (and alias `r`) ------------------------------------------------

#[test]
fn run_requires_output_and_uses_defaults() {
    let (config, define, run) = run_ok(&["tool", "run", "--output", "out.txt"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        run,
        RunArgs {
            output: "out.txt".to_owned(),
            trailing: vec![],
        }
    );
}

#[test]
fn run_alias_r_behaves_identically_for_success() {
    let via_run = run_ok(&["tool", "run", "--output", "o", "--", "a", "--flag"]).2;
    let via_r = run_ok(&["tool", "r", "--output", "o", "--", "a", "--flag"]).2;
    assert_eq!(via_run, via_r);
    assert_eq!(via_run.output, "o");
    assert_eq!(via_run.trailing, vec!["a", "--flag"]);
}

#[test]
fn run_double_dash_feeds_trailing_and_cannot_redispatch_externally() {
    for name in ["run", "r"] {
        let argv = [
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
        ];
        let (config, define, run) = run_ok(&argv);
        assert_eq!(run.output, "out.txt", "{name}");
        assert_eq!(
            run.trailing,
            vec!["plug", "--config", "other.toml", "run", "--", ""],
            "{name}"
        );
        // `run`'s separator terminates root parsing as well: its tail must not
        // re-enter the root or the external catch-all.
        assert_eq!(config, "builtin.toml", "{name}");
        assert_eq!(define, Vec::<String>::new(), "{name}");
    }
}

#[test]
fn globals_accepted_alongside_run_reach_root_state() {
    let (config, define, run) = run_ok(&[
        "tool", "run", "--config", "x.toml", "--output", "o", "--define", "k=v",
    ]);
    assert_eq!(config, "x.toml");
    assert_eq!(define, vec!["k=v"]);
    assert_eq!(run.output, "o");
}

// ---- failures -------------------------------------------------------------

#[test]
fn missing_output_fails_for_both_run_names_with_run_specific_context() {
    for argv in [&["tool", "run"][..], &["tool", "r"][..]] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::MissingRequiredArgument, "{argv:?}");
        assert!(msg.contains("--output"), "{argv:?}: {msg}");
        // The error must identify the most specific command, not the root.
        assert!(msg.contains("tool run"), "{argv:?}: {msg}");
        assert!(
            !msg.contains("tool [OPTIONS] <COMMAND>"),
            "{argv:?} unexpectedly used root usage:\n{msg}"
        );
    }
}

#[test]
fn duplicate_output_fails_for_both_run_names() {
    for argv in [
        &["tool", "run", "--output", "a", "--output", "b"][..],
        &["tool", "r", "--output", "a", "--output", "b"][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::ArgumentConflict, "{argv:?}");
        assert!(msg.contains("--output"), "{argv:?}: {msg}");
        assert!(msg.contains("tool run"), "{argv:?}: {msg}");
    }
}

#[test]
fn duplicate_root_config_fails_at_root() {
    let argv = ["tool", "--config", "a", "--config", "b", "plug"];
    let (kind, msg) = fail_kind(&argv);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--config"), "{msg}");
    assert!(msg.contains("tool [OPTIONS] <COMMAND>"), "{msg}");
}

#[test]
fn unknown_root_option_fails_on_triggering_token_before_external_name() {
    for argv in [
        &["tool", "--bogus", "plug"][..],
        &["tool", "--config", "c.toml", "--bogus", "plug"][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::UnknownArgument, "{argv:?}");
        assert!(msg.contains("--bogus"), "{argv:?}: {msg}");
        assert!(msg.contains("tool [OPTIONS] <COMMAND>"), "{argv:?}: {msg}");
    }

    let (kind, msg) = fail_kind(&["tool", "-x", "plug"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("-x"), "{msg}");
}

#[test]
fn unknown_run_option_fails_within_run_context() {
    let argv = ["tool", "run", "--output", "o", "--bogus"];
    let (kind, msg) = fail_kind(&argv);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--bogus"), "{msg}");
    assert!(msg.contains("tool run"), "{msg}");
}

#[test]
fn missing_option_value_fails_on_the_option() {
    for argv in [
        &["tool", "--config"][..],
        &["tool", "--define"][..],
        &["tool", "--config", "c.toml", "--define"][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::InvalidValue, "{argv:?}");
        let needle = if argv.last() == Some(&"--config") {
            "--config"
        } else {
            "--define"
        };
        assert!(msg.contains(needle), "{argv:?}: {msg}");
    }
}

#[test]
fn invalid_define_fails_on_value_before_external_name() {
    for argv in [
        &["tool", "--define", "nokey", "plug"][..],
        &["tool", "--define=nokey"][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("nokey"), "{argv:?}: {msg}");
        // The error names both the offending value and the argument it belongs to.
        assert!(msg.contains("--define"), "{argv:?}: {msg}");
    }
}

#[test]
fn invalid_define_after_external_name_is_captured_as_data() {
    let (config, define, tokens) = external_ok(&["tool", "plug", "--define", "nokey"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "--define", "nokey"]);
}

// ---- help -----------------------------------------------------------------

#[test]
fn root_help_succeeds_without_subcommand_or_required_values() {
    let err = parse(&["tool", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let help = rendered(&err);
    for needle in ["--config", "builtin.toml", "--define", "key=value", "run"] {
        assert!(help.contains(needle), "root help missing {needle}:\n{help}");
    }
    assert!(
        !help.contains("--output"),
        "run option leaked into root help:\n{help}"
    );
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[test]
fn run_help_succeeds_while_output_is_missing() {
    for argv in [&["tool", "run", "--help"][..], &["tool", "r", "--help"][..]] {
        let err = parse(argv).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp, "{argv:?}");
        let help = rendered(&err);
        assert!(help.contains("--output"), "{argv:?}: {help}");
        assert!(help.contains("tool run"), "{argv:?}: {help}");
        assert!(help.contains("TRAILING"), "{argv:?}: {help}");
        assert!(
            !help.to_ascii_lowercase().contains("error"),
            "{argv:?}: {help}"
        );
    }
}

// ---- fixed-order isolation sequence ---------------------------------------

#[test]
fn fixed_order_sequence_success_failure_success_help_is_isolated() {
    // Capture a clean baseline help up front; the trailing help requests must
    // render the same text after the intervening failure.
    let clean_root_help = rendered(&parse(&["tool", "--help"]).unwrap_err());
    let clean_run_help = rendered(&parse(&["tool", "run", "--help"]).unwrap_err());

    // 1. External success: explicit root state plus verbatim plugin tokens.
    let (config, define, tokens) = external_ok(&[
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

    // 2. Invalid root input fails categorically on the offending token.
    let err = parse(&["tool", "--define", "bogus", "plug"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ValueValidation);
    assert!(rendered(&err).contains("bogus"));

    // 3. `run` success is unaffected by either preceding call: root defaults
    //    are restored and no earlier tokens leak into trailing.
    let (config, define, run) = run_ok(&["tool", "r", "--output", "o", "--", "z", "--config"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        run,
        RunArgs {
            output: "o".to_owned(),
            trailing: vec!["z".to_owned(), "--config".to_owned()],
        }
    );

    // 4. Help requests still succeed and carry no trace of the failed call.
    let root_help = parse(&["tool", "--help"]).unwrap_err();
    assert_eq!(root_help.kind(), ErrorKind::DisplayHelp);
    let root_help = rendered(&root_help);
    assert_eq!(root_help, clean_root_help);
    assert!(!root_help.contains("bogus"), "{root_help}");
    assert!(!root_help.contains("--output"), "{root_help}");

    let run_help = parse(&["tool", "run", "--help"]).unwrap_err();
    assert_eq!(run_help.kind(), ErrorKind::DisplayHelp);
    let run_help = rendered(&run_help);
    assert_eq!(run_help, clean_run_help);
    assert!(!run_help.contains("bogus"), "{run_help}");
    assert!(run_help.contains("--output"), "{run_help}");
}
