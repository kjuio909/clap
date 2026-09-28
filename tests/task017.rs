//! Standalone, executable regression target for strict configuration
//! definitions and strict port value interpretation in a layered
//! plugin-dispatch CLI.
//!
//! Unlike the tests under `tests/derive/`, this file is a self-contained test
//! target that defines its own derived CLI and observes every result solely
//! through the public `clap::Parser` API via `Cli::try_parse_from` (argv0
//! included): successful structures, typed errors, and rendered help text.
//! Every case reports the actual tokens it ran with together with the
//! observed defines, port, error kind and triggering token, rather than only
//! contributing to an aggregate pass/fail.
//!
//! Scenario under test:
//! * root command `tool` with a global default config path and a root-level
//!   repeatable `--define` that accepts only exactly one non-empty key, one
//!   non-empty value and exactly one `=` separator, with no whitespace on
//!   either side of the pair;
//! * declared subcommands `serve` and `check`, plus a root external
//!   catch-all; `serve` declares `worker` and has its own external catch-all;
//! * `--define` is *not* global: it parses only while the root command is
//!   active, keeps occurrence order, is rejected with the triggering token at
//!   any deeper level, and is never parsed once either external capture
//!   starts;
//! * `worker` requires a unique `--port` built only from ASCII decimal digits
//!   in 1..=65535: empty strings, sign prefixes, hex, underscores, embedded
//!   whitespace and overflowing values are all reported as invalid values
//!   instead of being normalized, truncated or accepted;
//! * missing port, duplicate port, protocol conflicts and invalid values keep
//!   distinguishable error kinds that name the most specific command, while
//!   `check` and the plugin paths never inherit the port constraint;
//! * a fixed sequence of define success, every invalid-define class, port
//!   negatives, external success, a valid worker after a failure and
//!   four-level help is mutually isolated: help succeeds while required
//!   values are missing, carries no stale error, reflects only its own input,
//!   and keeps a stable ordering.
#![cfg(feature = "derive")]
#![cfg(feature = "help")]
#![cfg(feature = "usage")]

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::error::ErrorKind;

/// Strict `key=value` parser for root-level definitions.
///
/// Exactly one `=` separator, a non-empty key, a non-empty value and no
/// whitespace on either side of the pair. Empty keys, empty values, leading
/// or trailing `=`, additional `=` and whitespace around the pair are all
/// rejected instead of being trimmed, split further or silently accepted.
fn parse_define(raw: &str) -> Result<(String, String), String> {
    let Some((key, value)) = raw.split_once('=') else {
        return Err(format!("invalid define `{raw}`: expected key=value"));
    };
    if value.contains('=') {
        return Err(format!(
            "invalid define `{raw}`: expected exactly one `=` in key=value"
        ));
    }
    if key.is_empty() {
        return Err(format!("invalid define `{raw}`: key must not be empty"));
    }
    if value.is_empty() {
        return Err(format!("invalid define `{raw}`: value must not be empty"));
    }
    if key.chars().any(char::is_whitespace) || value.chars().any(char::is_whitespace) {
        return Err(format!(
            "invalid define `{raw}`: key and value must not contain whitespace"
        ));
    }
    Ok((key.to_owned(), value.to_owned()))
}

/// Strict decimal port parser: ASCII digits only, range 1..=65535.
///
/// Signs, hex prefixes, underscores, whitespace and empty values are all
/// rejected instead of being normalized, truncated or wrapped by a lenient
/// integer conversion.
fn parse_port(raw: &str) -> Result<u16, String> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!(
            "invalid port `{raw}`: expected a decimal number between 1 and 65535"
        ));
    }
    match raw.parse::<u32>() {
        Ok(port) if (1..=65535).contains(&port) => Ok(port as u16),
        _ => Err(format!(
            "invalid port `{raw}`: expected a decimal number between 1 and 65535"
        )),
    }
}

#[derive(Parser, Debug, Clone, PartialEq)]
#[command(
    name = "tool",
    after_help = "External plugins: any other <COMMAND> is captured as a root plugin."
)]
struct Cli {
    #[arg(
        long,
        global = true,
        default_value = "builtin.toml",
        value_name = "path"
    )]
    config: String,
    #[arg(long, value_name = "key=value", value_parser = parse_define)]
    define: Vec<(String, String)>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum Commands {
    Serve(ServeArgs),
    Check(CheckArgs),
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args, Debug, Clone, PartialEq)]
#[command(after_help = "External plugins: any other <COMMAND> is captured as a serve plugin.")]
struct ServeArgs {
    #[command(subcommand)]
    command: ServeCommands,
}

#[derive(Subcommand, Debug, Clone, PartialEq)]
enum ServeCommands {
    Worker(WorkerArgs),
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args, Debug, Clone, PartialEq)]
struct WorkerArgs {
    #[arg(long, value_name = "port", required = true, value_parser = parse_port)]
    port: u16,
    #[arg(long, conflicts_with = "udp")]
    tcp: bool,
    #[arg(long, conflicts_with = "tcp")]
    udp: bool,
    #[arg(last = true)]
    trailing: Vec<String>,
}

#[derive(Args, Debug, Clone, PartialEq)]
struct CheckArgs {
    path: Option<String>,
    #[arg(long)]
    strict: bool,
}

fn parse(argv: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(argv)
}

fn root_external_ok(argv: &[&str]) -> (String, Vec<(String, String)>, Vec<String>) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::External(tokens) = &cli.command else {
        panic!(
            "expected root external subcommand for {argv:?}, got {:?}",
            cli.command
        );
    };
    assert!(
        !tokens.is_empty(),
        "external capture must always include the command name: {argv:?}"
    );
    eprintln!(
        "{argv:?} => root plugin: config={:?} define={:?} tokens={:?}",
        cli.config, cli.define, tokens
    );
    (cli.config, cli.define, tokens.clone())
}

fn serve_external_ok(argv: &[&str]) -> (String, Vec<(String, String)>, Vec<String>) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::Serve(serve) = &cli.command else {
        panic!("expected `serve` for {argv:?}, got {:?}", cli.command);
    };
    let ServeCommands::External(tokens) = &serve.command else {
        panic!(
            "expected serve external subcommand for {argv:?}, got {:?}",
            serve.command
        );
    };
    assert!(
        !tokens.is_empty(),
        "external capture must always include the command name: {argv:?}"
    );
    eprintln!(
        "{argv:?} => serve plugin: config={:?} define={:?} tokens={:?}",
        cli.config, cli.define, tokens
    );
    (cli.config, cli.define, tokens.clone())
}

fn worker_ok(argv: &[&str]) -> (String, Vec<(String, String)>, WorkerArgs) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::Serve(serve) = &cli.command else {
        panic!(
            "expected `serve worker` for {argv:?}, got {:?}",
            cli.command
        );
    };
    let ServeCommands::Worker(worker) = &serve.command else {
        panic!("expected `worker` for {argv:?}, got {:?}", serve.command);
    };
    eprintln!(
        "{argv:?} => worker: config={:?} define={:?} port={} tcp={} udp={} trailing={:?}",
        cli.config, cli.define, worker.port, worker.tcp, worker.udp, worker.trailing
    );
    (cli.config, cli.define, worker.clone())
}

fn check_ok(argv: &[&str]) -> (String, Vec<(String, String)>, CheckArgs) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::Check(check) = &cli.command else {
        panic!("expected `check` for {argv:?}, got {:?}", cli.command);
    };
    eprintln!(
        "{argv:?} => check: config={:?} define={:?} path={:?} strict={}",
        cli.config, cli.define, check.path, check.strict
    );
    (cli.config, cli.define, check.clone())
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
        // A failure case that parses would leave a partially populated match
        // readable; the acceptance contract forbids that.
        Ok(cli) => panic!("expected failure for {argv:?}, got {cli:?}"),
        Err(err) => {
            let kind = err.kind();
            let msg = rendered(&err);
            let first_line = msg.lines().next().unwrap_or_default().to_owned();
            eprintln!("{argv:?} => error kind={kind:?}: {first_line}");
            (kind, msg)
        }
    }
}

fn help_text(argv: &[&str]) -> String {
    let err = parse(argv).expect_err("help is reported as a DisplayHelp error");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp, "{argv:?}");
    let help = rendered(&err);
    eprintln!("{argv:?} => help ({} bytes)", help.len());
    help
}

// ---- define success: strict pairs in occurrence order ---------------------

#[test]
fn defines_keep_occurrence_order_and_split_into_pairs() {
    let (config, define, worker) = worker_ok(&[
        "tool",
        "--define",
        "a=1",
        "--define",
        "b=two",
        "--define=c=3",
        "serve",
        "worker",
        "--port",
        "8080",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(
        define,
        vec![
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "two".to_owned()),
            ("c".to_owned(), "3".to_owned()),
        ]
    );
    assert_eq!(worker.port, 8080);
}

#[test]
fn define_pairs_keep_inner_punctuation_verbatim() {
    // Punctuation that is neither `=` nor whitespace is part of the data and
    // is never trimmed or reinterpreted.
    let (_, define, _) = worker_ok(&[
        "tool",
        "--define",
        "a.b-c_d=e/f:1",
        "--define",
        "x=y,z;w",
        "--define",
        "1=2",
        "serve",
        "worker",
        "--port",
        "80",
    ]);
    assert_eq!(
        define,
        vec![
            ("a.b-c_d".to_owned(), "e/f:1".to_owned()),
            ("x".to_owned(), "y,z;w".to_owned()),
            ("1".to_owned(), "2".to_owned()),
        ]
    );
}

#[test]
fn defines_parse_only_until_any_external_capture_starts() {
    // Root capture: defines before the name parse, everything after is data.
    let (_, define, tokens) = root_external_ok(&["tool", "--define", "k=v", "plug", "--define", "x=y"]);
    assert_eq!(define, vec![("k".to_owned(), "v".to_owned())]);
    assert_eq!(tokens, vec!["plug", "--define", "x=y"]);

    // Serve capture: a define between `serve` and the plugin name would be a
    // serve-level token and is covered by the wrong-level failures; once the
    // serve plugin name is selected, define-like tokens are plugin data.
    let (_, define, tokens) =
        serve_external_ok(&["tool", "--define", "k=v", "serve", "plug", "--define", "x=y"]);
    assert_eq!(define, vec![("k".to_owned(), "v".to_owned())]);
    assert_eq!(tokens, vec!["plug", "--define", "x=y"]);
}

// ---- invalid defines: one case per rejected class -------------------------

/// Every class of malformed definition, with the exact token under test.
const INVALID_DEFINES: &[&str] = &[
    "",        // empty string
    "=",       // only a separator: empty key and empty value
    "=v",      // leading equals: empty key
    "k=",      // trailing equals: empty value
    "a=b=c",   // more than one separator
    "novalue", // no separator at all
    " a=b",    // whitespace before the key
    "a =b",    // whitespace after the key
    "a= b",    // whitespace before the value
    "a=b ",    // whitespace after the value
    "a b=c",   // whitespace inside the key
    "a=b c",   // whitespace inside the value
];

#[test]
fn every_invalid_define_class_is_a_value_validation_error() {
    for raw in INVALID_DEFINES {
        for argv in [
            vec!["tool", "--define", raw, "serve", "worker", "--port", "80"],
            vec![
                "tool",
                format!("--define={raw}").as_str(),
                "serve",
                "worker",
                "--port",
                "80",
            ],
        ] {
            let (kind, msg) = fail_kind(&argv);
            assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
            assert!(msg.contains("--define"), "{argv:?}: {msg}");
            // The offending value is attributed verbatim, even when empty.
            assert!(msg.contains(&format!("'{raw}'")), "{argv:?}: {msg}");
        }
    }
}

#[test]
fn invalid_define_never_leaves_a_partial_match_at_any_level() {
    // The failure is attributed to `--define` and its value even when every
    // later token would have been valid on its own; no partial structure
    // (the check path, the strict flag) is readable from the result.
    let (kind, msg) = fail_kind(&["tool", "--define", "=v", "check", "path", "--strict"]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("--define"), "{msg}");
    assert!(msg.contains("'=v'"), "{msg}");
}

// ---- missing define values and option-like followers ----------------------

#[test]
fn omitted_define_value_fails_on_the_define_option() {
    let (kind, msg) = fail_kind(&["tool", "--define"]);
    assert_eq!(kind, ErrorKind::InvalidValue);
    assert!(msg.contains("--define"), "{msg}");
}

#[test]
fn next_option_is_never_mistaken_for_the_define_value() {
    for argv in [
        &["tool", "--define", "--config", "c.toml", "check"][..],
        &["tool", "--define", "--define", "a=1", "check"][..],
        &["tool", "--config", "c.toml", "--define", "--config", "d.toml", "check"][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::InvalidValue, "{argv:?}");
        assert!(msg.contains("--define"), "{argv:?}: {msg}");
    }
}

// ---- defines at the wrong level -------------------------------------------

#[test]
fn define_below_the_root_is_an_unknown_argument_naming_the_token() {
    let cases: &[(&[&str], &str)] = &[
        (
            &["tool", "serve", "--define", "a=1", "worker", "--port", "80"],
            "tool serve",
        ),
        (
            &["tool", "serve", "worker", "--port", "80", "--define", "a=1"],
            "tool serve worker",
        ),
        (&["tool", "check", "--define", "a=1"], "tool check"),
        // Even with a plugin name available, a serve-level define fails
        // instead of being captured as plugin data.
        (&["tool", "serve", "--define", "a=1", "plug"], "tool serve"),
    ];
    for (argv, usage) in cases {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::UnknownArgument, "{argv:?}");
        assert!(msg.contains("--define"), "{argv:?}: {msg}");
        assert!(msg.contains(usage), "{argv:?}: {msg}");
    }
}

// ---- port: strict decimal interpretation ----------------------------------

#[test]
fn port_accepts_decimal_bounds_only() {
    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port", "1"]);
    assert_eq!(worker.port, 1);
    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port=65535"]);
    assert_eq!(worker.port, 65535);
    // Leading zeroes are still plain decimal digits.
    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port", "080"]);
    assert_eq!(worker.port, 80);
}

#[test]
fn every_non_decimal_or_out_of_range_port_is_an_invalid_value() {
    // Attached form: every token reaches the port parser as its value.
    for bad in [
        "",                    // empty string
        "-1",                  // negative sign prefix
        "+80",                 // plus sign prefix
        "0x10",                // hexadecimal
        "1_000",               // digit separator
        " 80",                 // leading whitespace
        "80 ",                 // trailing whitespace
        "8 0",                 // embedded whitespace
        "abc",                 // non-numeric
        "0",                   // below the range
        "65536",               // above the range
        "99999999999999999999", // overflows any machine integer
    ] {
        let argv = ["tool", "serve", "worker", &format!("--port={bad}")];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}: {msg}");
        // The offending value is attributed verbatim, even when empty.
        assert!(msg.contains(&format!("'{bad}'")), "{argv:?}: {msg}");
    }

    // Detached form: tokens that do not look like flags are consumed as the
    // value and rejected by the same parser, never loosely converted.
    for bad in ["", "+80", "0x10", "1_000", " 80", "80 ", "abc", "0", "65536"] {
        let argv = ["tool", "serve", "worker", "--port", bad];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}: {msg}");
    }

    // A detached negative number is consumed by the option lexer as a flag
    // and is therefore reported as the unknown token it actually is; it is
    // still rejected, never accepted or truncated.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "-1"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("-1"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
}

#[test]
fn missing_duplicate_and_conflicting_port_arguments_keep_distinct_kinds() {
    let (kind, msg) = fail_kind(&["tool", "serve", "worker"]);
    assert_eq!(kind, ErrorKind::MissingRequiredArgument);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
    assert!(!msg.contains("tool check"), "{msg}");

    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "80", "--port", "90"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    let (kind, msg) =
        fail_kind(&["tool", "serve", "worker", "--port", "80", "--tcp", "--udp"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--tcp"), "{msg}");
    assert!(msg.contains("--udp"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    // The three categories stay distinguishable from each other and from an
    // invalid value.
    let (invalid, _) = fail_kind(&["tool", "serve", "worker", "--port", "0"]);
    assert_eq!(invalid, ErrorKind::ValueValidation);
    assert_ne!(ErrorKind::MissingRequiredArgument, ErrorKind::ArgumentConflict);
    assert_ne!(ErrorKind::MissingRequiredArgument, invalid);
    assert_ne!(ErrorKind::ArgumentConflict, invalid);
}

#[test]
fn check_and_plugin_paths_do_not_inherit_the_port_constraint() {
    // Numeric-looking paths are plain data for `check`.
    let (_, _, check) = check_ok(&["tool", "check", "65536"]);
    assert_eq!(check.path.as_deref(), Some("65536"));

    // The port option itself is unknown to `check`, not reinterpreted.
    let (kind, msg) = fail_kind(&["tool", "check", "--port", "80"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool check"), "{msg}");
    assert!(!msg.contains("tool serve worker"), "{msg}");

    // Plugin captures pass port-shaped tokens through verbatim.
    let (_, define, tokens) = root_external_ok(&["tool", "plug", "--port", "0", "--define", "bad"]);
    assert_eq!(define, Vec::<(String, String)>::new());
    assert_eq!(tokens, vec!["plug", "--port", "0", "--define", "bad"]);

    let (_, define, tokens) = serve_external_ok(&["tool", "serve", "plug", "--port", "abc"]);
    assert_eq!(define, Vec::<(String, String)>::new());
    assert_eq!(tokens, vec!["plug", "--port", "abc"]);
}

// ---- external command success ----------------------------------------------

#[test]
fn external_captures_keep_every_token_after_the_name_verbatim() {
    let (config, define, tokens) = root_external_ok(&[
        "tool",
        "--define",
        "a=1",
        "--config",
        "custom.toml",
        "plug",
        "--config",
        "stolen.toml",
        "--define",
        "=v",
        "serve",
        "worker",
        "--",
        "",
    ]);
    assert_eq!(config, "custom.toml");
    assert_eq!(define, vec![("a".to_owned(), "1".to_owned())]);
    assert_eq!(
        tokens,
        vec![
            "plug",
            "--config",
            "stolen.toml",
            "--define",
            "=v",
            "serve",
            "worker",
            "--",
            "",
        ]
    );

    let (config, define, tokens) = serve_external_ok(&[
        "tool",
        "--define",
        "b=2",
        "serve",
        "plug",
        "worker",
        "--port",
        "0",
        "check",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, vec![("b".to_owned(), "2".to_owned())]);
    assert_eq!(tokens, vec!["plug", "worker", "--port", "0", "check"]);
}

// ---- global config position compatibility ---------------------------------

#[test]
fn global_config_parses_at_any_position_without_changing_the_structure() {
    let pairs: &[(&[&str], &[&str])] = &[
        (
            &["tool", "--config", "c.toml", "serve", "worker", "--port", "80"],
            &["tool", "serve", "worker", "--port", "80", "--config", "c.toml"],
        ),
        (
            &["tool", "--config", "c.toml", "check", "p", "--strict"],
            &["tool", "check", "p", "--strict", "--config", "c.toml"],
        ),
    ];
    for (left, right) in pairs {
        let a = parse(left).unwrap_or_else(|err| panic!("left failed {left:?}: {err}"));
        let b = parse(right).unwrap_or_else(|err| panic!("right failed {right:?}: {err}"));
        assert_eq!(a, b, "structure differs for {left:?} vs {right:?}");
    }

    // After an external name, `--config` is plugin data, not a global.
    let (config, _, tokens) = root_external_ok(&["tool", "--config", "c.toml", "plug", "--config", "x"]);
    assert_eq!(config, "c.toml");
    assert_eq!(tokens, vec!["plug", "--config", "x"]);

    // Duplicated global config is still a conflict at the level where the
    // second occurrence appears.
    let (kind, msg) = fail_kind(&[
        "tool", "serve", "worker", "--port", "80", "--config", "a", "--config", "b",
    ]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--config"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
}

// ---- help at all four levels ------------------------------------------------

#[test]
fn root_help_succeeds_and_shows_only_root_surface() {
    let help = help_text(&["tool", "--help"]);
    for needle in [
        "--config",
        "<path>",
        "builtin.toml",
        "--define",
        "<key=value>",
        "serve",
        "check",
        "captured as a root plugin",
    ] {
        assert!(help.contains(needle), "root help missing {needle}:\n{help}");
    }
    for leaked in ["--port", "--tcp", "--udp", "--strict", "TRAILING", "worker"] {
        assert!(
            !help.contains(leaked),
            "root help unexpectedly contains {leaked}:\n{help}"
        );
    }
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[test]
fn serve_help_succeeds_without_define_surface() {
    let help = help_text(&["tool", "serve", "--help"]);
    for needle in [
        "worker",
        "--config",
        "builtin.toml",
        "tool serve",
        "captured as a serve plugin",
    ] {
        assert!(help.contains(needle), "serve help missing {needle}:\n{help}");
    }
    // `--define` belongs to the root alone and must not leak downwards.
    for leaked in ["--define", "--port", "--tcp", "--udp", "--strict"] {
        assert!(
            !help.contains(leaked),
            "serve help unexpectedly contains {leaked}:\n{help}"
        );
    }
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[test]
fn worker_help_succeeds_while_port_is_missing() {
    let help = help_text(&["tool", "serve", "worker", "--help"]);
    for needle in [
        "--port",
        "<port>",
        "--tcp",
        "--udp",
        "TRAILING",
        "--config",
        "builtin.toml",
        "tool serve worker",
    ] {
        assert!(help.contains(needle), "worker help missing {needle}:\n{help}");
    }
    for leaked in ["--define", "--strict", "root plugin", "serve plugin"] {
        assert!(
            !help.contains(leaked),
            "worker help unexpectedly contains {leaked}:\n{help}"
        );
    }
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[test]
fn check_help_succeeds_without_path_and_shows_only_check_surface() {
    let help = help_text(&["tool", "check", "--help"]);
    for needle in ["--strict", "PATH", "--config", "builtin.toml", "tool check"] {
        assert!(help.contains(needle), "check help missing {needle}:\n{help}");
    }
    for leaked in [
        "--define",
        "--port",
        "--tcp",
        "--udp",
        "TRAILING",
        "worker",
        "root plugin",
        "serve plugin",
    ] {
        assert!(
            !help.contains(leaked),
            "check help unexpectedly contains {leaked}:\n{help}"
        );
    }
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[test]
fn help_ordering_is_stable_across_repeated_requests() {
    for argv in [
        &["tool", "--help"][..],
        &["tool", "serve", "--help"][..],
        &["tool", "serve", "worker", "--help"][..],
        &["tool", "check", "--help"][..],
    ] {
        assert_eq!(
            help_text(argv),
            help_text(argv),
            "help rendering is not stable for {argv:?}"
        );
    }
}

// ---- fixed-order isolation sequence -----------------------------------------

#[test]
fn fixed_order_sequence_defines_ports_externals_worker_help_is_isolated() {
    // Clean baselines captured before anything else runs; the trailing help
    // requests must render byte-identical text after the intervening calls.
    let clean_root_help = help_text(&["tool", "--help"]);
    let clean_serve_help = help_text(&["tool", "serve", "--help"]);
    let clean_worker_help = help_text(&["tool", "serve", "worker", "--help"]);
    let clean_check_help = help_text(&["tool", "check", "--help"]);

    // 1. Define success: strict pairs land in the structure in order.
    let (config, define, worker) = worker_ok(&[
        "tool",
        "--define",
        "a=1",
        "--define",
        "b=2",
        "serve",
        "worker",
        "--port",
        "8080",
        "--tcp",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(
        define,
        vec![("a".to_owned(), "1".to_owned()), ("b".to_owned(), "2".to_owned())]
    );
    assert_eq!(worker.port, 8080);
    assert!(worker.tcp);

    // 2. Every invalid-define class fails on its own token.
    for raw in INVALID_DEFINES {
        let argv = ["tool", "--define", raw, "check"];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--define"), "{argv:?}: {msg}");
    }

    // 3. Port negatives: invalid values, never coerced.
    for bad in ["", "-1", "+80", "0x10", "1_000", " 80", "0", "65536"] {
        let argv = ["tool", "serve", "worker", &format!("--port={bad}")];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}: {msg}");
    }

    // 4. External command success in a fresh parse: no define or port state
    //    leaks from the failing calls, and plugin tokens stay verbatim.
    let (config, define, tokens) = root_external_ok(&[
        "tool", "--define", "k=v", "plug", "--define", "=v", "--port", "0",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, vec![("k".to_owned(), "v".to_owned())]);
    assert_eq!(tokens, vec!["plug", "--define", "=v", "--port", "0"]);

    // 5. A valid worker parse after the failures sees only its own input.
    let (config, define, worker) = worker_ok(&[
        "tool", "serve", "worker", "--port", "9090", "--udp", "--", "z", "--define",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<(String, String)>::new());
    assert_eq!(
        worker,
        WorkerArgs {
            port: 9090,
            tcp: false,
            udp: true,
            trailing: vec!["z".to_owned(), "--define".to_owned()],
        }
    );

    // 6. Every help request still succeeds while required values are missing,
    //    is identical to the clean baselines, and carries no stale error.
    for (argv, clean) in [
        (&["tool", "--help"][..], &clean_root_help),
        (&["tool", "serve", "--help"][..], &clean_serve_help),
        (&["tool", "serve", "worker", "--help"][..], &clean_worker_help),
        (&["tool", "check", "--help"][..], &clean_check_help),
    ] {
        let help = help_text(argv);
        assert_eq!(&help, clean, "{argv:?} help changed across the sequence");
        assert!(!help.contains("novalue"), "{argv:?}:\n{help}");
        assert!(!help.contains("0x10"), "{argv:?}:\n{help}");
    }
}
