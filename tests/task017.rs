//! Standalone, executable regression target for strict configuration-definition
//! and port-value validation on top of the layered subcommand dispatcher.
//!
//! Like `task016.rs`, this file is self-contained: it defines its own derived
//! CLI and observes every result solely through the public `clap::Parser` API
//! via `Cli::try_parse_from` (argv0 included): successful structures, typed
//! errors, and rendered help text.
//!
//! Scenario under test (refining `task016`):
//! * root command `tool` keeps the defaulted, global `--config <path>` (so it
//!   still parses on either side of every declared command token) and a
//!   repeatable, **root-level-only** `--define <key=value>`;
//! * a definition must contain exactly one `=`, splitting one non-empty key and
//!   one non-empty value with no ASCII whitespace next to the separator: no
//!   equals, leading/trailing equals, more than one equals, empty key or value,
//!   and whitespace beside the key or value are all rejected as invalid values
//!   instead of reaching the business layer;
//! * definitions save in occurrence order at the root only; using `--define`
//!   under `serve`, `worker` or `check` is an unknown token of that level, and
//!   nothing at or after either external-plugin name is ever parsed again;
//! * omitting the definition value, or letting the next option be mistaken for
//!   the value, fails while naming the triggering token;
//! * `worker --port <port>` accepts ASCII decimal digits only and only
//!   1..=65535: empty strings, signs, hex, underscores, whitespace and
//!   overflows are invalid values without coercion or truncation;
//! * missing, duplicate, conflicting and invalid `--port` inputs keep their
//!   distinct error kinds and the most specific command, and neither `check`
//!   nor any plugin path inherits the port;
//! * a fixed sequence (define success, every invalid define class, port
//!   negatives, external success, a valid `worker` after a failure, and the
//!   four help levels) is mutually isolated, and usage/help ordering is stable
//!   with no stale error carried between parses.
#![cfg(feature = "derive")]
#![cfg(feature = "help")]
#![cfg(feature = "usage")]

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::error::ErrorKind;

/// Strict decimal port parser: ASCII digits only, range 1..=65535.
///
/// Signs, hex prefixes, underscores, whitespace and empty values are all
/// rejected instead of being normalized by `u16::from_str`; leading zeroes are
/// plain decimal digits and accepted (`"080"` is port 80).
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

/// Strict definition parser: exactly one `=`, non-empty key and value, with no
/// ASCII whitespace adjacent to either side of the separator.
fn parse_define(raw: &str) -> Result<String, String> {
    let Some((key, value)) = raw.split_once('=') else {
        return Err(format!("invalid define `{raw}`: expected key=value"));
    };
    let boundary_whitespace = |s: &str| {
        s.starts_with(|c: char| c.is_ascii_whitespace())
            || s.ends_with(|c: char| c.is_ascii_whitespace())
    };
    if key.is_empty()
        || value.is_empty()
        || value.contains('=')
        || boundary_whitespace(key)
        || boundary_whitespace(value)
    {
        Err(format!("invalid define `{raw}`: expected key=value"))
    } else {
        Ok(raw.to_owned())
    }
}

#[derive(Parser, Debug, Clone, PartialEq)]
#[command(
    name = "tool",
    after_help = "External plugins: any other <COMMAND> is captured as a root plugin."
)]
struct Cli {
    // `--config` stays global so its left/right placement keeps working.
    #[arg(
        long,
        global = true,
        default_value = "builtin.toml",
        value_name = "path"
    )]
    config: String,
    // `--define` is intentionally root-level only: subcommands never inherit it.
    #[arg(
        long,
        value_name = "key=value",
        value_parser = parse_define,
        allow_hyphen_values = true
    )]
    define: Vec<String>,
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
    #[arg(
        long,
        value_name = "port",
        required = true,
        value_parser = parse_port,
        allow_hyphen_values = true
    )]
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

fn root_external_ok(argv: &[&str]) -> (String, Vec<String>, Vec<String>) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::External(tokens) = cli.command else {
        panic!(
            "expected root external subcommand for {argv:?}, got {:?}",
            cli.command
        );
    };
    assert!(
        !tokens.is_empty(),
        "external capture must always include the command name: {argv:?}"
    );
    (cli.config, cli.define, tokens)
}

fn serve_external_ok(argv: &[&str]) -> (String, Vec<String>, Vec<String>) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::Serve(serve) = cli.command else {
        panic!("expected `serve` for {argv:?}, got {:?}", cli.command);
    };
    let ServeCommands::External(tokens) = serve.command else {
        panic!(
            "expected serve external subcommand for {argv:?}, got {:?}",
            serve.command
        );
    };
    assert!(
        !tokens.is_empty(),
        "external capture must always include the command name: {argv:?}"
    );
    (cli.config, cli.define, tokens)
}

fn worker_ok(argv: &[&str]) -> (String, Vec<String>, WorkerArgs) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::Serve(serve) = cli.command else {
        panic!(
            "expected `serve worker` for {argv:?}, got {:?}",
            cli.command
        );
    };
    let ServeCommands::Worker(worker) = serve.command else {
        panic!("expected `worker` for {argv:?}, got {:?}", serve.command);
    };
    (cli.config, cli.define, worker)
}

fn check_ok(argv: &[&str]) -> (String, Vec<String>, CheckArgs) {
    let cli = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let Commands::Check(check) = cli.command else {
        panic!("expected `check` for {argv:?}, got {:?}", cli.command);
    };
    (cli.config, cli.define, check)
}

/// Render an error while dropping the optional backtrace appended by clap's
/// `debug` feature. The backtrace captures host paths and frame line numbers
/// and is not part of the user-facing help or error output under test.
fn rendered(err: &clap::Error) -> String {
    let text = err.to_string();
    match text.split_once("\nBacktrace:") {
        Some((head, _)) => head.trim_end().to_owned(),
        None => text.trim_end().to_owned(),
    }
}

fn fail_kind(argv: &[&str]) -> (ErrorKind, String) {
    match parse(argv) {
        Ok(cli) => panic!("expected failure for {argv:?}, got {cli:?}"),
        Err(err) => (err.kind(), rendered(&err)),
    }
}

fn help_text(argv: &[&str]) -> String {
    let err = parse(argv).expect_err("help is reported as a DisplayHelp error");
    assert_eq!(err.kind(), ErrorKind::DisplayHelp, "{argv:?}");
    rendered(&err)
}

// ---- definition success ----------------------------------------------------

#[test]
fn root_definitions_save_strict_pairs_in_occurrence_order() {
    let (config, define, tokens) = root_external_ok(&[
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

    // Whitespace away from the separator is data, not padding: the rule only
    // bans whitespace on the two sides of the `=`.
    let (_, define, _) = root_external_ok(&["tool", "--define", "a b=c d", "plug"]);
    assert_eq!(define, vec!["a b=c d"]);
    let (_, define, _) = root_external_ok(&["tool", "--define", "msg=hello world", "plug"]);
    assert_eq!(define, vec!["msg=hello world"]);

    // Root definitions precede the declared command tokens at every level.
    let (config, define, worker) =
        worker_ok(&["tool", "--define", "a=1", "serve", "worker", "--port", "80"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, vec!["a=1"]);
    assert_eq!(worker.port, 80);

    let (_, define, check) = check_ok(&["tool", "--define", "k=v", "check", "p"]);
    assert_eq!(define, vec!["k=v"]);
    assert_eq!(check.path.as_deref(), Some("p"));

    let (_, define, tokens) = serve_external_ok(&["tool", "--define", "a=1", "serve", "plug", "x"]);
    assert_eq!(define, vec!["a=1"]);
    assert_eq!(tokens, vec!["plug", "x"]);
}

// ---- every class of invalid definition -------------------------------------

#[test]
fn invalid_define_classes_fail_as_value_validation_naming_value_and_option() {
    // Each row is one offending definition value, exercised in three shapes:
    // detached with a following plugin name, attached with a following plugin
    // name, and the attached form standing alone (which must not fall through
    // to "missing subcommand" instead of validating).
    const BAD: &[&str] = &[
        "nokey", // no equals at all
        "=v",    // leading equals: empty key
        "k=",    // trailing equals: empty value
        "=",     // only the separator: empty key and value
        "a=b=c", // more than one equals
        " a=1",  // whitespace beside the key
        "a =1", "a=1 ", // whitespace beside the value
        "a= 1", "a\t=1", // tabs count as whitespace too
        "a=1\t", " ", // neither side salvageable
    ];
    for bad in BAD {
        // Detached form with a following plugin name: the value is validated
        // before the plugin name is consumed.
        let detached = ["tool", "--define", bad, "plug"];
        let (kind, msg) = fail_kind(&detached);
        assert_eq!(kind, ErrorKind::ValueValidation, "{detached:?}");
        assert!(msg.contains("--define"), "{detached:?}:\n{msg}");
        assert!(msg.contains(bad), "{detached:?}:\n{msg}");
        assert!(msg.contains("expected key=value"), "{detached:?}:\n{msg}");

        // Attached forms cannot carry interior whitespace the way a single
        // detached argv element can, so exercise them for single-token values.
        if !bad.chars().any(|c| c.is_ascii_whitespace()) {
            let with_plugin = format!("--define={bad}");
            let argv = ["tool", with_plugin.as_str(), "plug"];
            let (kind, msg) = fail_kind(&argv);
            assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
            assert!(msg.contains("--define"), "{argv:?}:\n{msg}");
            assert!(msg.contains(bad), "{argv:?}:\n{msg}");

            // The attached form standing alone must still validate instead of
            // falling through to "missing subcommand".
            let argv = ["tool", with_plugin.as_str()];
            let (kind, msg) = fail_kind(&argv);
            assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
            assert!(msg.contains("--define"), "{argv:?}:\n{msg}");
            assert!(msg.contains(bad), "{argv:?}:\n{msg}");
        }
    }

    // The empty value is attributed explicitly in both detached and attached
    // shapes instead of being dropped or treated as a missing token.
    let (kind, msg) = fail_kind(&["tool", "--define", "", "plug"]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("invalid value ''"), "{msg}");
    assert!(msg.contains("--define"), "{msg}");

    let (kind, msg) = fail_kind(&["tool", "--define="]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("invalid value ''"), "{msg}");
}

// ---- definitions at the wrong level ----------------------------------------

#[test]
fn define_is_unknown_below_the_root_and_names_that_level() {
    // `serve` and its plugin slot never accept a root definition.
    for argv in [
        &["tool", "serve", "--define", "a=1", "plug"][..],
        &["tool", "serve", "--define", "a=1", "worker", "--port", "80"][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::UnknownArgument, "{argv:?}");
        assert!(msg.contains("--define"), "{argv:?}:\n{msg}");
        assert!(msg.contains("tool serve"), "{argv:?}:\n{msg}");
    }

    // Inside worker the option itself is the unknown token.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "80", "--define", "a=1"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--define"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    // The same holds for check, even for an invalid pair: the level is checked
    // before any value parsing could happen.
    for argv in [
        &["tool", "check", "--define", "a=1"][..],
        &["tool", "check", "--define", "bad"][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::UnknownArgument, "{argv:?}");
        assert!(msg.contains("--define"), "{argv:?}:\n{msg}");
        assert!(msg.contains("tool check"), "{argv:?}:\n{msg}");
        assert!(!msg.contains("tool serve worker"), "{argv:?}:\n{msg}");
    }

    // A definition appearing only after the descent token is wrong-level even
    // when an earlier, root-side definition was valid.
    let (kind, _) = fail_kind(&[
        "tool", "--define", "a=1", "serve", "worker", "--port", "80", "--define", "b=2",
    ]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
}

// ---- omitted or swallowed definition values --------------------------------

#[test]
fn missing_or_swallowed_define_value_fails_on_the_triggering_token() {
    // Omitted entirely: no value follows.
    let (kind, msg) = fail_kind(&["tool", "--define"]);
    assert_eq!(kind, ErrorKind::InvalidValue);
    assert!(msg.contains("--define"), "{msg}");

    // The next option is consumed as the value and rejected, naming both the
    // swallowed token and the option it was fed to.
    let (kind, msg) = fail_kind(&["tool", "--define", "--config", "x", "plug"]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("'--config'"), "{msg}");
    assert!(msg.contains("--define"), "{msg}");

    let (kind, msg) = fail_kind(&["tool", "--define", "--bogus", "plug"]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("'--bogus'"), "{msg}");

    // A missing value at the end of a completed command path is still the
    // option's error, not a subcommand failure.
    let (kind, msg) = fail_kind(&["tool", "check", "--config", "c.toml", "--define"]);
    assert_eq!(kind, ErrorKind::UnknownArgument, "{msg}");
    assert!(msg.contains("--define"), "{msg}");
}

// ---- external capture freezes every following token ------------------------

#[test]
fn after_an_external_name_tokens_are_never_definitions_or_ports() {
    let (config, define, tokens) =
        root_external_ok(&["tool", "--define", "a=1", "plug", "--define", "b="]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, vec!["a=1"]);
    assert_eq!(tokens, vec!["plug", "--define", "b="]);

    // Invalid pairs and port-looking data after the name are plugin data.
    let (_, define, tokens) = root_external_ok(&[
        "tool", "plug", "--define", "a=b=c", "--port", "0", "serve", "worker",
    ]);
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        tokens,
        vec![
            "plug", "--define", "a=b=c", "--port", "0", "serve", "worker"
        ]
    );

    let (_, define, tokens) =
        serve_external_ok(&["tool", "serve", "plug", "--define", "=x", "--port", "-1"]);
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "--define", "=x", "--port", "-1"]);

    // The root `--` makes even option-looking tokens plugin data.
    let (_, define, tokens) = root_external_ok(&["tool", "--", "--define", "a=1"]);
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["--define", "a=1"]);
}

// ---- port success -----------------------------------------------------------

#[test]
fn worker_port_accepts_decimal_bounds_and_leading_zeroes() {
    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port", "1", "--tcp"]);
    assert_eq!(worker.port, 1);
    assert!(worker.tcp);
    assert!(!worker.udp);
    assert_eq!(worker.trailing, Vec::<String>::new());

    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port=65535", "--udp"]);
    assert_eq!(worker.port, 65535);
    assert!(worker.udp);

    // Leading zeroes are still plain decimal digits, not octal.
    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port", "080"]);
    assert_eq!(worker.port, 80);
}

// ---- port negative classes --------------------------------------------------

#[test]
fn invalid_port_values_are_value_validation_without_coercion() {
    const BAD: &[&str] = &[
        "",                     // empty
        "-1",                   // negative sign (detached and attached)
        "+80",                  // explicit plus
        "0x10",                 // hex prefix
        "ff",                   // hex digits without prefix
        "8_0",                  // thousands underscore
        "8 0",                  // embedded whitespace
        " 80",                  // leading whitespace
        "80\t",                 // trailing tab
        "0",                    // below range
        "65536",                // one above range
        "99999999999999999999", // far beyond u16/u32
        "１２",                 // non-ASCII digits are not decimal ASCII
    ];
    for bad in BAD {
        let argv = ["tool", "serve", "worker", "--port", bad];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}:\n{msg}");
        if bad.is_empty() {
            assert!(msg.contains("invalid value ''"), "{argv:?}:\n{msg}");
        } else {
            assert!(msg.contains(bad), "{argv:?}:\n{msg}");
        }

        // The attached form reaches the parser identically.
        let attached = format!("--port={bad}");
        let argv = ["tool", "serve", "worker", &attached];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}:\n{msg}");
        if bad.is_empty() {
            assert!(msg.contains("invalid value ''"), "{argv:?}:\n{msg}");
        } else {
            assert!(msg.contains(bad), "{argv:?}:\n{msg}");
        }
    }
}

// ---- distinct port error categories and level isolation ---------------------

#[test]
fn missing_duplicate_and_conflicting_ports_keep_distinct_kinds_and_context() {
    // Missing required port.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker"]);
    assert_eq!(kind, ErrorKind::MissingRequiredArgument);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
    assert!(!msg.contains("tool check"), "{msg}");

    // Missing value for the present option.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port"]);
    assert_eq!(kind, ErrorKind::InvalidValue);
    assert!(msg.contains("--port"), "{msg}");

    // Duplicate port.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "80", "--port", "90"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    // Protocol conflict remains its own class with an otherwise valid port.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "80", "--tcp", "--udp"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--tcp"), "{msg}");
    assert!(msg.contains("--udp"), "{msg}");
}

#[test]
fn check_root_and_plugin_paths_do_not_inherit_the_port() {
    // check: a numeric-looking bare token is its path, and --port is unknown.
    let (_, _, check) = check_ok(&["tool", "check", "80"]);
    assert_eq!(check.path.as_deref(), Some("80"));

    let (kind, msg) = fail_kind(&["tool", "check", "--port", "80"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool check"), "{msg}");
    assert!(!msg.contains("tool serve worker"), "{msg}");

    // Root does not know worker's port either.
    let (kind, msg) = fail_kind(&["tool", "--port", "80", "plug"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool [OPTIONS] <COMMAND>"), "{msg}");

    // Plugin paths capture port-looking tokens verbatim with no validation.
    let (_, _, tokens) = root_external_ok(&["tool", "plug", "--port", "0"]);
    assert_eq!(tokens, vec!["plug", "--port", "0"]);
    let (_, _, tokens) = serve_external_ok(&["tool", "serve", "plug", "--port", "-1"]);
    assert_eq!(tokens, vec!["plug", "--port", "-1"]);
}

// ---- declared precedence and separators (no capture regressions) -----------

#[test]
fn declared_commands_win_but_separators_and_prefixes_stay_external() {
    let (_, _, tokens) = root_external_ok(&["tool", "ser", "x"]);
    assert_eq!(tokens, vec!["ser", "x"]);
    let (_, _, tokens) = serve_external_ok(&["tool", "serve", "work", "x"]);
    assert_eq!(tokens, vec!["work", "x"]);
    let (_, _, tokens) = root_external_ok(&["tool", "worker", "--port", "8080"]);
    assert_eq!(tokens, vec!["worker", "--port", "8080"]);

    let (_, _, tokens) = root_external_ok(&["tool", "--", "serve", "x"]);
    assert_eq!(tokens, vec!["serve", "x"]);
    let (_, _, tokens) = serve_external_ok(&["tool", "serve", "--", "worker", "--port", "1"]);
    assert_eq!(tokens, vec!["worker", "--port", "1"]);

    // Worker's own separator yields trailing arguments, never redispatch.
    let (_, _, worker) = worker_ok(&[
        "tool", "serve", "worker", "--port", "8080", "--tcp", "--", "plug", "--port", "9090",
        "serve", "--", "",
    ]);
    assert_eq!(worker.port, 8080);
    assert!(worker.tcp);
    assert_eq!(
        worker.trailing,
        vec!["plug", "--port", "9090", "serve", "--", ""]
    );

    // Without the separator worker still rejects a bare positional.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "8080", "plug"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("'plug'"), "{msg}");
}

// ---- global config placement keeps working ----------------------------------

#[test]
fn global_config_still_flanks_every_command_token() {
    let (config, _, worker) = worker_ok(&[
        "tool",
        "serve",
        "worker",
        "--port",
        "80",
        "--config",
        "deep.toml",
    ]);
    assert_eq!(config, "deep.toml");
    assert_eq!(worker.port, 80);

    let (config, _, _) = check_ok(&["tool", "check", "p", "--config=c.toml", "--strict"]);
    assert_eq!(config, "c.toml");

    let (config, define, tokens) =
        root_external_ok(&["tool", "--config=x.toml", "--define", "a=1", "plug"]);
    assert_eq!(config, "x.toml");
    assert_eq!(define, vec!["a=1"]);
    assert_eq!(tokens, vec!["plug"]);
}

// ---- help -------------------------------------------------------------------

#[test]
fn root_help_lists_root_surface_in_stable_order() {
    let help = help_text(&["tool", "--help"]);
    for needle in [
        "Usage: tool [OPTIONS] <COMMAND>",
        "--config <path>",
        "builtin.toml",
        "--define <key=value>",
        "serve",
        "check",
        "captured as a root plugin",
    ] {
        assert!(help.contains(needle), "root help missing {needle}:\n{help}");
    }
    for leaked in [
        "--port",
        "--tcp",
        "--udp",
        "--strict",
        "TRAILING",
        "worker",
        "captured as a serve plugin",
    ] {
        assert!(
            !help.contains(leaked),
            "root help unexpectedly contains {leaked}:\n{help}"
        );
    }
    // Option ordering: config precedes define within the options block, and
    // both precede the external-plugin footer.
    let pos = |needle: &str| help.find(needle).unwrap_or_else(|| panic!("{needle}"));
    assert!(pos("--config <path>") < pos("--define <key=value>"));
    assert!(pos("--define <key=value>") < pos("captured as a root plugin"));
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[test]
fn serve_help_hides_define_and_worker_surface() {
    let help = help_text(&["tool", "serve", "--help"]);
    for needle in [
        "Usage: tool serve [OPTIONS] <COMMAND>",
        "worker",
        "--config <path>",
        "builtin.toml",
        "captured as a serve plugin",
    ] {
        assert!(
            help.contains(needle),
            "serve help missing {needle}:\n{help}"
        );
    }
    for leaked in [
        "--define",
        "--port",
        "--tcp",
        "--udp",
        "--strict",
        "[PATH]",
        "captured as a root plugin",
    ] {
        assert!(
            !help.contains(leaked),
            "serve help unexpectedly contains {leaked}:\n{help}"
        );
    }
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[test]
fn worker_help_succeeds_while_port_is_missing_without_define() {
    let help = help_text(&["tool", "serve", "worker", "--help"]);
    for needle in [
        "Usage: tool serve worker [OPTIONS] --port <port>",
        "--port <port>",
        "--tcp",
        "--udp",
        "TRAILING",
        "--config <path>",
        "builtin.toml",
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
    for needle in [
        "Usage: tool check [OPTIONS] [PATH]",
        "--strict",
        "PATH",
        "--config <path>",
        "builtin.toml",
        "tool check",
    ] {
        assert!(
            help.contains(needle),
            "check help missing {needle}:\n{help}"
        );
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

// ---- fixed-order, per-case isolation sequence -------------------------------

#[test]
fn fixed_order_sequence_defines_port_negatives_external_worker_help_is_isolated() {
    // Clean help baselines captured first; the four help renders at the end
    // must be byte-identical despite every intervening parse.
    let clean_root_help = help_text(&["tool", "--help"]);
    let clean_serve_help = help_text(&["tool", "serve", "--help"]);
    let clean_worker_help = help_text(&["tool", "serve", "worker", "--help"]);
    let clean_check_help = help_text(&["tool", "check", "--help"]);

    // 1. Definition success: strict pairs save in order on the root state.
    let tokens: &[&str] = &[
        "tool",
        "--config",
        "custom.toml",
        "--define",
        "a=1",
        "--define",
        "b=2",
        "rootplug",
        "--flag",
    ];
    let (config, define, captured) = root_external_ok(tokens);
    assert_eq!(config, "custom.toml", "{tokens:?}");
    assert_eq!(define, vec!["a=1", "b=2"], "{tokens:?}");
    assert_eq!(captured, vec!["rootplug", "--flag"], "{tokens:?}");

    // 2. Every invalid definition class, fixed order: the offending value and
    //    `--define` are both named and nothing partial is observable.
    for bad in ["nokey", "=v", "k=", "=", "a=b=c", " a=1", "a=1 "] {
        let argv = ["tool", "--define", bad, "plug"];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains(bad), "{argv:?}:\n{msg}");
        assert!(msg.contains("--define"), "{argv:?}:\n{msg}");
    }
    let (kind, msg) = fail_kind(&["tool", "--define="]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("invalid value ''"), "{msg}");
    let (kind, msg) = fail_kind(&["tool", "--define"]);
    assert_eq!(kind, ErrorKind::InvalidValue);
    assert!(msg.contains("--define"), "{msg}");
    let (kind, msg) = fail_kind(&["tool", "--define", "--config", "x", "plug"]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("'--config'"), "{msg}");
    let (kind, _) = fail_kind(&["tool", "serve", "--define", "a=1", "plug"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);

    // 3. Port negatives, fixed order: each is a value validation naming the
    //    exact token, distinct from missing/duplicate/conflict.
    for bad in [
        "",
        "-1",
        "+80",
        "0x10",
        "8_0",
        "8 0",
        "0",
        "65536",
        "99999999999999999999",
    ] {
        let argv = ["tool", "serve", "worker", "--port", bad];
        let (kind, msg) = fail_kind(&argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}:\n{msg}");
        if bad.is_empty() {
            assert!(msg.contains("invalid value ''"), "{argv:?}:\n{msg}");
        } else {
            assert!(msg.contains(bad), "{argv:?}:\n{msg}");
        }
    }
    let (kind, _) = fail_kind(&["tool", "serve", "worker"]);
    assert_eq!(kind, ErrorKind::MissingRequiredArgument);
    let (kind, _) = fail_kind(&["tool", "serve", "worker", "--port", "80", "--port", "90"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);

    // 4. External command success: earlier failures leave no trace, and a
    //    port-looking token after the plugin name is verbatim data.
    let (config, define, captured) =
        serve_external_ok(&["tool", "serve", "serveplug", "--port", "0"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(captured, vec!["serveplug", "--port", "0"]);

    // 5. A valid worker immediately after the failures parses cleanly: root
    //    defaults are back, the port is accepted and no token leaked in.
    let (config, define, worker) =
        worker_ok(&["tool", "serve", "worker", "--port", "8080", "--udp"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        worker,
        WorkerArgs {
            port: 8080,
            tcp: false,
            udp: true,
            trailing: vec![],
        }
    );

    // 6. All four help levels still succeed, are byte-identical to the clean
    //    baselines, and carry no stale offending token.
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
        for stale in ["a=b=c", "99999999999999999999", "0x10"] {
            assert!(!help.contains(stale), "{argv:?}:\n{help}");
        }
    }
}
