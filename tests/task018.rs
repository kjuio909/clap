//! Standalone, executable regression target for the builder production
//! path: global configuration, repeatable definitions, nested subcommands
//! and external-plugin dispatch must all resolve into one stable
//! `ArgMatches` through the public `clap::Command` builder API.
//!
//! Unlike the sibling targets, this file never constructs a derive-based
//! `Cli` struct: the command tree is assembled with `Command::new`/`Arg::new`
//! and every result is observed solely through the public entry point
//! [`clap::Command::try_get_matches_from`] (argv0 included) - successful
//! `ArgMatches`, typed errors and rendered help text. This is the path the
//! real application takes, so values that must never reach the business
//! layer cannot slip past behind a test-only struct shape.
//!
//! Scenario under test:
//! * root command `tool` with a global `--config <path>` defaulting to
//!   `builtin.toml` and a root-level repeatable `--define <key=value>` that
//!   accepts only exactly one non-empty key, one non-empty value and exactly
//!   one `=` separator, with no whitespace on either side of the pair;
//! * declared subcommands `serve` and `check`, plus a root external
//!   catch-all; `serve` declares `worker` and has its own external catch-all;
//! * `--define` parses only while the root command is active, keeps
//!   occurrence order, is rejected with the triggering token at any deeper
//!   level, and is never parsed once either external capture starts;
//! * `worker` requires a unique `--port <port>` built only from ASCII
//!   decimal digits in 1..=65535: empty strings, sign prefixes, hex,
//!   underscores, embedded whitespace and overflowing values are reported as
//!   invalid values instead of being normalized, truncated or accepted;
//! * `--http` and `--https` are mutually exclusive; tokens after the
//!   worker's own `--` only become ordered trailing arguments;
//! * `check` accepts only an optional path and `--strict` and never consumes
//!   worker-specific arguments;
//! * missing port, duplicate port, protocol conflicts and invalid values
//!   keep distinguishable error kinds that name the most specific command
//!   and the triggering token, and a failure never yields a readable match;
//! * root, serve, worker and check help succeed even while required values
//!   are missing, showing only that level's surface, defaults, subcommands
//!   and external-capture boundary, with no stale error mixed in;
//! * a fixed sequence of external success, every invalid-define class,
//!   port failures, a valid worker after a failure and help requests is
//!   mutually isolated, and moving the global arguments around changes
//!   neither the success data, define order, trailing order, error usage
//!   nor the help order.
#![cfg(feature = "help")]
#![cfg(feature = "usage")]

use clap::Arg;
use clap::ArgAction;
use clap::ArgMatches;
use clap::Command;
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

/// The production command tree, built fresh from the public builder API for
/// every parse, exactly as the application constructs it.
fn cli() -> Command {
    Command::new("tool")
        .subcommand_required(true)
        .after_help("External plugins: any other <COMMAND> is captured as a root plugin.")
        .arg(
            Arg::new("config")
                .long("config")
                .global(true)
                .default_value("builtin.toml")
                .value_name("path"),
        )
        .arg(
            Arg::new("define")
                .long("define")
                .value_name("key=value")
                .action(ArgAction::Append)
                .value_parser(parse_define),
        )
        .subcommand(
            Command::new("serve")
                .subcommand_required(true)
                .after_help("External plugins: any other <COMMAND> is captured as a serve plugin.")
                .allow_external_subcommands(true)
                .external_subcommand_value_parser(clap::value_parser!(String))
                .subcommand(
                    Command::new("worker")
                        .arg(
                            Arg::new("port")
                                .long("port")
                                .value_name("port")
                                .required(true)
                                .value_parser(parse_port),
                        )
                        .arg(
                            Arg::new("http")
                                .long("http")
                                .action(ArgAction::SetTrue)
                                .conflicts_with("https"),
                        )
                        .arg(
                            Arg::new("https")
                                .long("https")
                                .action(ArgAction::SetTrue)
                                .conflicts_with("http"),
                        )
                        .arg(
                            Arg::new("trailing")
                                .value_name("TRAILING")
                                .num_args(1..)
                                .action(ArgAction::Append)
                                .last(true),
                        ),
                ),
        )
        .subcommand(
            Command::new("check")
                .arg(Arg::new("path").value_name("PATH"))
                .arg(Arg::new("strict").long("strict").action(ArgAction::SetTrue)),
        )
        .allow_external_subcommands(true)
        .external_subcommand_value_parser(clap::value_parser!(String))
}

/// Successful worker match projected into plain data the business layer would
/// read; never constructed on a failure path.
#[derive(Debug, Clone, PartialEq)]
struct Worker {
    port: u16,
    http: bool,
    https: bool,
    trailing: Vec<String>,
}

fn parse(argv: &[&str]) -> Result<ArgMatches, clap::Error> {
    cli().try_get_matches_from(argv)
}

fn root_config(matches: &ArgMatches) -> String {
    matches
        .get_one::<String>("config")
        .expect("global config always has a value")
        .clone()
}

fn root_defines(matches: &ArgMatches) -> Vec<(String, String)> {
    matches
        .get_many::<(String, String)>("define")
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

/// Reconstruct a plugin token vector the way the dispatcher reads it: the
/// captured subcommand name followed by every verbatim following token.
fn external_tokens(name: &str, plugin: &ArgMatches) -> Vec<String> {
    let mut tokens = vec![name.to_owned()];
    tokens.extend(
        plugin
            .get_many::<String>("")
            .expect("external capture holds the following tokens")
            .cloned(),
    );
    tokens
}

fn root_external_ok(argv: &[&str]) -> (String, Vec<(String, String)>, Vec<String>) {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let (name, plugin) = matches.subcommand().expect("a subcommand on success");
    assert!(
        name != "serve" && name != "check",
        "expected a root plugin for {argv:?}, got declared subcommand `{name}`"
    );
    let tokens = external_tokens(name, plugin);
    assert!(
        !tokens.is_empty(),
        "external capture must always include the command name: {argv:?}"
    );
    eprintln!(
        "{argv:?} => root plugin: config={:?} define={:?} tokens={:?}",
        root_config(&matches),
        root_defines(&matches),
        tokens
    );
    (root_config(&matches), root_defines(&matches), tokens)
}

fn serve_external_ok(argv: &[&str]) -> (String, Vec<(String, String)>, Vec<String>) {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Some(("serve", serve)) = matches.subcommand() else {
        panic!(
            "expected `serve` for {argv:?}, got {:?}",
            matches.subcommand().map(|(n, _)| n)
        );
    };
    let (name, plugin) = serve.subcommand().expect("serve requires a subcommand");
    assert_ne!(
        name, "worker",
        "expected a serve plugin for {argv:?}, got worker"
    );
    let tokens = external_tokens(name, plugin);
    assert!(
        !tokens.is_empty(),
        "external capture must always include the command name: {argv:?}"
    );
    eprintln!(
        "{argv:?} => serve plugin: config={:?} define={:?} tokens={:?}",
        root_config(&matches),
        root_defines(&matches),
        tokens
    );
    (root_config(&matches), root_defines(&matches), tokens)
}

fn worker_matches<'a>(matches: &'a ArgMatches, argv: &[&str]) -> &'a ArgMatches {
    let Some(("serve", serve)) = matches.subcommand() else {
        panic!(
            "expected `serve worker` for {argv:?}, got {:?}",
            matches.subcommand().map(|(n, _)| n)
        );
    };
    let Some(("worker", worker)) = serve.subcommand() else {
        panic!(
            "expected `worker` for {argv:?}, got {:?}",
            serve.subcommand().map(|(n, _)| n)
        );
    };
    worker
}

fn worker_ok(argv: &[&str]) -> (String, Vec<(String, String)>, Worker) {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let worker_m = worker_matches(&matches, argv);
    let worker = Worker {
        port: *worker_m
            .get_one::<u16>("port")
            .expect("required port on success"),
        http: worker_m.get_flag("http"),
        https: worker_m.get_flag("https"),
        trailing: worker_m
            .get_many::<String>("trailing")
            .map(|values| values.cloned().collect())
            .unwrap_or_default(),
    };
    eprintln!(
        "{argv:?} => worker: config={:?} define={:?} port={} http={} https={} trailing={:?}",
        root_config(&matches),
        root_defines(&matches),
        worker.port,
        worker.http,
        worker.https,
        worker.trailing
    );
    (root_config(&matches), root_defines(&matches), worker)
}

fn check_ok(argv: &[&str]) -> (String, Vec<(String, String)>, Option<String>, bool) {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Some(("check", check)) = matches.subcommand() else {
        panic!(
            "expected `check` for {argv:?}, got {:?}",
            matches.subcommand().map(|(n, _)| n)
        );
    };
    let path = check.get_one::<String>("path").cloned();
    let strict = check.get_flag("strict");
    eprintln!(
        "{argv:?} => check: config={:?} define={:?} path={path:?} strict={strict}",
        root_config(&matches),
        root_defines(&matches)
    );
    (root_config(&matches), root_defines(&matches), path, strict)
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
        // A failure case that matches would leave a partially populated match
        // readable; the acceptance contract forbids that.
        Ok(matches) => panic!("expected failure for {argv:?}, got {matches:?}"),
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
    let (_, define, tokens) =
        root_external_ok(&["tool", "--define", "k=v", "plug", "--define", "x=y"]);
    assert_eq!(define, vec![("k".to_owned(), "v".to_owned())]);
    assert_eq!(tokens, vec!["plug", "--define", "x=y"]);

    // Serve capture: a define between `serve` and the plugin name would be a
    // serve-level token and is covered by the wrong-level failures; once the
    // serve plugin name is selected, define-like tokens are plugin data.
    let (_, define, tokens) = serve_external_ok(&[
        "tool", "--define", "k=v", "serve", "plug", "--define", "x=y",
    ]);
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
        &[
            "tool", "--config", "c.toml", "--define", "--config", "d.toml", "check",
        ][..],
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
        "",                     // empty string
        "-1",                   // negative sign prefix
        "+80",                  // plus sign prefix
        "0x10",                 // hexadecimal
        "1_000",                // digit separator
        " 80",                  // leading whitespace
        "80 ",                  // trailing whitespace
        "8 0",                  // embedded whitespace
        "abc",                  // non-numeric
        "0",                    // below the range
        "65536",                // above the range
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
    for bad in [
        "", "+80", "0x10", "1_000", " 80", "80 ", "abc", "0", "65536",
    ] {
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

    let (kind, msg) = fail_kind(&[
        "tool", "serve", "worker", "--port", "80", "--http", "--https",
    ]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--http"), "{msg}");
    assert!(msg.contains("--https"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    // The three categories stay distinguishable from each other and from an
    // invalid value.
    let (invalid, _) = fail_kind(&["tool", "serve", "worker", "--port", "0"]);
    assert_eq!(invalid, ErrorKind::ValueValidation);
    assert_ne!(
        ErrorKind::MissingRequiredArgument,
        ErrorKind::ArgumentConflict
    );
    assert_ne!(ErrorKind::MissingRequiredArgument, invalid);
    assert_ne!(ErrorKind::ArgumentConflict, invalid);
}

#[test]
fn check_and_plugin_paths_do_not_inherit_the_port_constraint() {
    // Numeric-looking paths are plain data for `check`.
    let (_, _, path, strict) = check_ok(&["tool", "check", "65536"]);
    assert_eq!(path.as_deref(), Some("65536"));
    assert!(!strict);

    // `check` keeps its optional path plus the strict switch and nothing else.
    let (_, _, path, strict) = check_ok(&["tool", "check", "p", "--strict"]);
    assert_eq!(path.as_deref(), Some("p"));
    assert!(strict);

    // The port option itself is unknown to `check`, not reinterpreted.
    let (kind, msg) = fail_kind(&["tool", "check", "--port", "80"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool check"), "{msg}");
    assert!(!msg.contains("tool serve worker"), "{msg}");

    // A second positional is not silently swallowed either.
    let (kind, _) = fail_kind(&["tool", "check", "p", "q"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);

    // Plugin captures pass port-shaped tokens through verbatim.
    let (_, define, tokens) = root_external_ok(&["tool", "plug", "--port", "0", "--define", "bad"]);
    assert_eq!(define, Vec::<(String, String)>::new());
    assert_eq!(tokens, vec!["plug", "--port", "0", "--define", "bad"]);

    let (_, define, tokens) = serve_external_ok(&["tool", "serve", "plug", "--port", "abc"]);
    assert_eq!(define, Vec::<(String, String)>::new());
    assert_eq!(tokens, vec!["plug", "--port", "abc"]);
}

// ---- worker trailing arguments ---------------------------------------------

#[test]
fn worker_double_dash_feeds_ordered_trailing_arguments_only() {
    // Everything after the worker's own `--` is trailing data in order,
    // including option-looking tokens, declared command names and a second
    // separator; protocol switches there stay data and never flip flags.
    let (_, _, worker) = worker_ok(&[
        "tool", "serve", "worker", "--port", "9090", "--https", "--", "z", "--http", "serve",
        "check", "--", "",
    ]);
    assert_eq!(worker.port, 9090);
    assert!(worker.https);
    assert!(!worker.http);
    assert_eq!(
        worker.trailing,
        vec!["z", "--http", "serve", "check", "--", ""]
    );
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
        "tool", "--define", "b=2", "serve", "plug", "worker", "--port", "0", "check",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, vec![("b".to_owned(), "2".to_owned())]);
    assert_eq!(tokens, vec!["plug", "worker", "--port", "0", "check"]);
}

// ---- global config position compatibility ---------------------------------

#[test]
fn global_config_parses_at_any_position_without_changing_the_data() {
    // Moving only the global option's position changes none of the data the
    // business layer reads: config, define order, port and flags. (The moved
    // token never crosses a `--` boundary; after the worker separator every
    // token is trailing data by contract.)
    let pairs: &[(&[&str], &[&str])] = &[
        (
            &[
                "tool", "--config", "c.toml", "--define", "a=1", "serve", "worker", "--port", "80",
                "--http",
            ],
            &[
                "tool", "--define", "a=1", "serve", "worker", "--port", "80", "--http", "--config",
                "c.toml",
            ],
        ),
        (
            &["tool", "--config", "c.toml", "check", "p", "--strict"],
            &["tool", "check", "p", "--strict", "--config", "c.toml"],
        ),
    ];
    for (left, right) in pairs {
        let a = success_snapshot(left);
        let b = success_snapshot(right);
        assert_eq!(a, b, "snapshot differs for {left:?} vs {right:?}");
    }

    // After an external name, `--config` is plugin data, not a global.
    let (config, _, tokens) =
        root_external_ok(&["tool", "--config", "c.toml", "plug", "--config", "x"]);
    assert_eq!(config, "c.toml");
    assert_eq!(tokens, vec!["plug", "--config", "x"]);

    // Duplicated global config is still a conflict at the level where the
    // second occurrence appears, regardless of position.
    let (kind, msg) = fail_kind(&[
        "tool", "serve", "worker", "--port", "80", "--config", "a", "--config", "b",
    ]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--config"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
}

/// Position-independent snapshot of every readable value on the worker or
/// check success paths.
#[derive(Debug, PartialEq)]
struct SuccessSnapshot {
    config: String,
    define: Vec<(String, String)>,
    worker: Option<Worker>,
    check: Option<Check>,
}

#[derive(Debug, PartialEq)]
struct Check {
    path: Option<String>,
    strict: bool,
}

fn success_snapshot(argv: &[&str]) -> SuccessSnapshot {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let config = root_config(&matches);
    let define = root_defines(&matches);
    let (name, sub) = matches.subcommand().expect("subcommand on success path");
    match name {
        "serve" => {
            let worker_m = sub.subcommand().expect("worker").1;
            let worker = Worker {
                port: *worker_m.get_one::<u16>("port").unwrap(),
                http: worker_m.get_flag("http"),
                https: worker_m.get_flag("https"),
                trailing: worker_m
                    .get_many::<String>("trailing")
                    .map(|values| values.cloned().collect())
                    .unwrap_or_default(),
            };
            SuccessSnapshot {
                config,
                define,
                worker: Some(worker),
                check: None,
            }
        }
        "check" => SuccessSnapshot {
            config,
            define,
            worker: None,
            check: Some(Check {
                path: sub.get_one::<String>("path").cloned(),
                strict: sub.get_flag("strict"),
            }),
        },
        other => panic!("unexpected subcommand {other} for {argv:?}"),
    }
}

#[test]
fn moving_globals_does_not_change_error_usage() {
    // The same unknown/conflict failure renders identically with the global
    // option placed before or after the subcommand tokens.
    let cases: &[(&[&str], &[&str])] = &[
        (
            &[
                "tool", "--config", "c.toml", "serve", "worker", "--port", "80", "--bogus",
            ],
            &[
                "tool", "serve", "worker", "--port", "80", "--bogus", "--config", "c.toml",
            ],
        ),
        (
            &["tool", "--config", "c.toml", "check", "--bogus"],
            &["tool", "check", "--bogus", "--config", "c.toml"],
        ),
    ];
    for (left, right) in cases {
        let l = fail_kind(left);
        let r = fail_kind(right);
        assert_eq!(l.0, r.0, "{left:?} vs {right:?}");
        assert_eq!(
            l.1, r.1,
            "error rendering differs for {left:?} vs {right:?}"
        );
    }
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
    for leaked in [
        "--port", "--http", "--https", "--strict", "TRAILING", "worker",
    ] {
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
        assert!(
            help.contains(needle),
            "serve help missing {needle}:\n{help}"
        );
    }
    // `--define` belongs to the root alone and must not leak downwards.
    for leaked in ["--define", "--port", "--http", "--https", "--strict"] {
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
        "--http",
        "--https",
        "TRAILING",
        "--config",
        "builtin.toml",
        "tool serve worker",
    ] {
        assert!(
            help.contains(needle),
            "worker help missing {needle}:\n{help}"
        );
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
        assert!(
            help.contains(needle),
            "check help missing {needle}:\n{help}"
        );
    }
    for leaked in [
        "--define",
        "--port",
        "--http",
        "--https",
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
fn help_ordering_is_stable_across_repeated_requests_and_global_positions() {
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

    // A global token placed before the help request changes nothing in the
    // rendered ordering or content.
    assert_eq!(
        help_text(&["tool", "--help"]),
        help_text(&["tool", "--config", "c.toml", "--help"])
    );
    assert_eq!(
        help_text(&["tool", "serve", "worker", "--help"]),
        help_text(&["tool", "serve", "worker", "--config", "c.toml", "--help"])
    );
}

// ---- fixed-order isolation sequence -----------------------------------------

#[test]
fn fixed_order_sequence_externals_defines_ports_worker_help_is_isolated() {
    // Clean baselines captured before anything else runs; the trailing help
    // requests must render byte-identical text after the intervening calls.
    let clean_root_help = help_text(&["tool", "--help"]);
    let clean_serve_help = help_text(&["tool", "serve", "--help"]);
    let clean_worker_help = help_text(&["tool", "serve", "worker", "--help"]);
    let clean_check_help = help_text(&["tool", "check", "--help"]);

    // 1. External success plus strict definitions: plugin tokens stay
    //    verbatim while root data lands in the match in order.
    let (config, define, tokens) = root_external_ok(&[
        "tool",
        "--define",
        "a=1",
        "--config",
        "custom.toml",
        "plug",
        "--define",
        "=v",
        "--port",
        "0",
        "--http",
    ]);
    assert_eq!(config, "custom.toml");
    assert_eq!(define, vec![("a".to_owned(), "1".to_owned())]);
    assert_eq!(
        tokens,
        vec!["plug", "--define", "=v", "--port", "0", "--http"]
    );

    // A serve-level plugin is isolated from the root plugin parse.
    let (config, define, tokens) =
        serve_external_ok(&["tool", "serve", "plug", "worker", "--https"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<(String, String)>::new());
    assert_eq!(tokens, vec!["plug", "worker", "--https"]);

    // 2. Every invalid-define class fails on its own token with no match.
    for raw in INVALID_DEFINES {
        let argv = ["tool", "--define", raw, "check"];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--define"), "{argv:?}: {msg}");
        assert!(msg.contains(&format!("'{raw}'")), "{argv:?}: {msg}");
    }

    // 3. Port negatives: invalid values, never coerced.
    for bad in ["", "-1", "+80", "0x10", "1_000", " 80", "0", "65536"] {
        let argv = ["tool", "serve", "worker", &format!("--port={bad}")];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}: {msg}");
    }
    // Missing, duplicate and conflicting ports stay their own categories.
    assert_eq!(
        fail_kind(&["tool", "serve", "worker"]).0,
        ErrorKind::MissingRequiredArgument
    );
    assert_eq!(
        fail_kind(&["tool", "serve", "worker", "--port", "80", "--port", "90"]).0,
        ErrorKind::ArgumentConflict
    );
    assert_eq!(
        fail_kind(&[
            "tool", "serve", "worker", "--port", "80", "--http", "--https"
        ])
        .0,
        ErrorKind::ArgumentConflict
    );

    // 4. A valid worker parse after the failures sees only its own input;
    //    the worker `--` tail keeps order and treats option-like tokens as
    //    plain trailing data.
    let (config, define, worker) = worker_ok(&[
        "tool", "serve", "worker", "--port", "9090", "--https", "--", "z", "--define",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<(String, String)>::new());
    assert_eq!(
        worker,
        Worker {
            port: 9090,
            http: false,
            https: true,
            trailing: vec!["z".to_owned(), "--define".to_owned()],
        }
    );

    // 5. Every help request still succeeds while required values are
    //    missing, is identical to the clean baselines, and carries no stale
    //    error or token from the failed calls.
    for (argv, clean) in [
        (&["tool", "--help"][..], &clean_root_help),
        (&["tool", "serve", "--help"][..], &clean_serve_help),
        (
            &["tool", "serve", "worker", "--help"][..],
            &clean_worker_help,
        ),
        (&["tool", "check", "--help"][..], &clean_check_help),
    ] {
        let help = help_text(argv);
        assert_eq!(&help, clean, "{argv:?} help changed across the sequence");
        assert!(!help.contains("novalue"), "{argv:?}:\n{help}");
        assert!(!help.contains("0x10"), "{argv:?}:\n{help}");
        assert!(
            !help.to_ascii_lowercase().contains("error"),
            "{argv:?}:\n{help}"
        );
    }
}
