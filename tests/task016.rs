//! Standalone, executable regression target for layered subcommand dispatch
//! with external-plugin catch-alls at two levels.
//!
//! Unlike the tests under `tests/derive/`, this file is a self-contained test
//! target that defines its own derived CLI and observes every result solely
//! through the public `clap::Parser` API via `Cli::try_parse_from` (argv0
//! included): successful structures, typed errors, and rendered help text.
//!
//! Scenario under test:
//! * root command `tool` with a default config path and repeatable
//!   `--define` items, both global so they parse next to any subcommand;
//! * declared subcommands `serve` and `check`, plus a root external
//!   catch-all for root-level plugins;
//! * `serve` declares `worker` and has its own external catch-all for serve
//!   plugins, so an unknown name under `serve` never escapes to the root;
//! * `worker` requires a unique decimal `--port` in 1..=65535, two mutually
//!   exclusive protocol switches (`--tcp`/`--udp`), and verbatim trailing
//!   arguments after its own `--`;
//! * `check` takes only an optional path and `--strict` and must never accept
//!   arguments owned by `serve` or `worker`;
//! * globals may appear before or after the root/serve/worker/check tokens;
//!   explicit config overrides the default and defines keep occurrence order;
//! * declared subcommands win over external capture, and once an external
//!   name is selected every following token belongs to that plugin;
//! * a root-level `--` forces the next token to be the root plugin name, a
//!   serve-level `--` forces the next token to be the serve plugin name, while
//!   the separator inside `worker` only produces trailing arguments;
//! * validation failures name the triggering token (and argument), use the
//!   distinct error kinds for missing, invalid, conflicting and unknown
//!   tokens, and never leave a partially populated match readable;
//! * help for root, serve, worker and check succeeds even while required
//!   values are missing and shows only that level's surface;
//! * a fixed sequence of root-plugin success, serve-plugin success, invalid
//!   input, worker success and help requests is mutually isolated, and merely
//!   moving the global arguments around changes neither the parsed structure,
//!   trailing order, error usage nor the order seen in help.
#![cfg(feature = "derive")]
#![cfg(feature = "help")]
#![cfg(feature = "usage")]

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::error::ErrorKind;

/// Strict decimal port parser: digits only, range 1..=65535.
///
/// Signs, hex prefixes, whitespace and empty values are all rejected instead
/// of being normalized by `u16::from_str`.
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

fn parse_define(raw: &str) -> Result<String, String> {
    if raw.contains('=') {
        Ok(raw.to_owned())
    } else {
        Err(format!("invalid define `{raw}`: expected key=value"))
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
    #[arg(long, global = true, value_name = "key=value", value_parser = parse_define)]
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
        None => text,
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

// ---- declared subcommands take precedence over external capture ----------

#[test]
fn declared_commands_are_not_swallowed_by_external_capture() {
    // Root-level declared names dispatch to the built-in commands.
    assert!(matches!(
        parse(&["tool", "check"]).unwrap().command,
        Commands::Check(_)
    ));
    assert!(matches!(
        parse(&["tool", "serve", "worker", "--port", "8080"])
            .unwrap()
            .command,
        Commands::Serve(ServeArgs {
            command: ServeCommands::Worker(_),
        })
    ));

    // Prefixes and look-alikes at either level stay external.
    let (_, _, tokens) = root_external_ok(&["tool", "ser", "x"]);
    assert_eq!(tokens, vec!["ser", "x"]);
    let (_, _, tokens) = serve_external_ok(&["tool", "serve", "work", "x"]);
    assert_eq!(tokens, vec!["work", "x"]);
    let (_, _, tokens) = serve_external_ok(&["tool", "serve", "check"]);
    assert_eq!(tokens, vec!["check"]);

    // A bare `worker` at the root is a root plugin, never the nested command.
    let (_, _, tokens) = root_external_ok(&["tool", "worker", "--port", "8080"]);
    assert_eq!(tokens, vec!["worker", "--port", "8080"]);
}

#[test]
fn missing_subcommand_shows_that_levels_help_without_dispatching() {
    // Neither level may guess a plugin name when no token is present.
    let (kind, _) = fail_kind(&["tool"]);
    assert_eq!(kind, ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand);
    let (kind, msg) = fail_kind(&["tool", "serve"]);
    assert_eq!(kind, ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand);
    assert!(msg.contains("tool serve"), "{msg}");
    assert!(msg.contains("worker"), "{msg}");
}

// ---- root external dispatch ----------------------------------------------

#[test]
fn root_external_uses_root_defaults() {
    let (config, define, tokens) = root_external_ok(&["tool", "plug", "arg"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "arg"]);
}

#[test]
fn root_explicit_config_overrides_default_and_defines_keep_order() {
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
}

#[test]
fn root_external_captures_every_token_after_name_verbatim() {
    let (config, define, tokens) = root_external_ok(&[
        "tool",
        "--config",
        "custom.toml",
        "plug",
        "--config",
        "stolen.toml",
        "--define",
        "bad",
        "serve",
        "worker",
        "--",
        "",
    ]);
    assert_eq!(config, "custom.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        tokens,
        vec![
            "plug",
            "--config",
            "stolen.toml",
            "--define",
            "bad",
            "serve",
            "worker",
            "--",
            "",
        ]
    );
}

// ---- serve external dispatch ---------------------------------------------

#[test]
fn serve_external_uses_root_defaults_and_stays_under_serve() {
    let (config, define, tokens) = serve_external_ok(&["tool", "serve", "plug", "arg"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "arg"]);
}

#[test]
fn serve_globals_before_the_plugin_name_reach_root_state() {
    let (config, define, tokens) = serve_external_ok(&[
        "tool", "--config", "c.toml", "serve", "--define", "a=1", "plug", "x",
    ]);
    assert_eq!(config, "c.toml");
    assert_eq!(define, vec!["a=1"]);
    assert_eq!(tokens, vec!["plug", "x"]);
}

#[test]
fn serve_external_tokens_after_name_are_not_stolen_by_any_level() {
    let (config, define, tokens) = serve_external_ok(&[
        "tool",
        "serve",
        "plug",
        "--config",
        "stolen.toml",
        "--define",
        "bad",
        "worker",
        "--port",
        "0",
        "check",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        tokens,
        vec![
            "plug",
            "--config",
            "stolen.toml",
            "--define",
            "bad",
            "worker",
            "--port",
            "0",
            "check",
        ]
    );
}

// ---- root- and serve-level `--` -------------------------------------------

#[test]
fn root_double_dash_forces_next_token_to_be_root_plugin_name() {
    let (config, define, tokens) =
        root_external_ok(&["tool", "--config", "c.toml", "--", "--flag", "x", "--", ""]);
    assert_eq!(config, "c.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["--flag", "x", "--", ""]);

    // Declared names after the separator are plugin names, not commands.
    let (_, _, tokens) = root_external_ok(&["tool", "--", "serve", "x"]);
    assert_eq!(tokens, vec!["serve", "x"]);
    let (_, _, tokens) = root_external_ok(&["tool", "--", "check"]);
    assert_eq!(tokens, vec!["check"]);
    let (_, _, tokens) = root_external_ok(&["tool", "--", "", "rest"]);
    assert_eq!(tokens, vec!["", "rest"]);
}

#[test]
fn serve_double_dash_forces_next_token_to_be_serve_plugin_name() {
    let (config, define, tokens) = serve_external_ok(&[
        "tool", "serve", "--define", "a=1", "--", "worker", "--port", "1",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, vec!["a=1"]);
    assert_eq!(tokens, vec!["worker", "--port", "1"]);

    let (_, _, tokens) = serve_external_ok(&["tool", "serve", "--", "--flag"]);
    assert_eq!(tokens, vec!["--flag"]);
    let (_, _, tokens) = serve_external_ok(&["tool", "serve", "--", "", "x"]);
    assert_eq!(tokens, vec!["", "x"]);
}

// ---- worker success -------------------------------------------------------

#[test]
fn worker_requires_port_on_success_path_and_accepts_bounds() {
    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port", "1", "--tcp"]);
    assert_eq!(
        worker,
        WorkerArgs {
            port: 1,
            tcp: true,
            udp: false,
            trailing: vec![],
        }
    );

    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port=65535", "--udp"]);
    assert_eq!(worker.port, 65535);
    assert!(worker.udp);

    // Leading zeroes are still plain decimal digits.
    let (_, _, worker) = worker_ok(&["tool", "serve", "worker", "--port", "080"]);
    assert_eq!(worker.port, 80);
}

#[test]
fn worker_trailing_after_separator_is_verbatim_and_never_redispatches() {
    let (config, define, worker) = worker_ok(&[
        "tool", "serve", "worker", "--port", "8080", "--tcp", "--", "plug", "--tcp", "--udp",
        "--port", "9090", "worker", "serve", "--", "",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(worker.port, 8080);
    assert!(worker.tcp);
    assert!(!worker.udp);
    assert_eq!(
        worker.trailing,
        vec![
            "plug", "--tcp", "--udp", "--port", "9090", "worker", "serve", "--", "",
        ]
    );
}

#[test]
fn worker_trailing_requires_the_separator() {
    // Without `--`, worker has no positional slot and a bare token is rejected
    // rather than silently treated as trailing data.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "8080", "plug"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("'plug'"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
}

// ---- check success --------------------------------------------------------

#[test]
fn check_takes_one_optional_path_and_strict() {
    let (config, define, check) = check_ok(&["tool", "check"]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        check,
        CheckArgs {
            path: None,
            strict: false,
        }
    );

    let (_, _, check) = check_ok(&["tool", "check", "--strict", "path/to/file"]);
    assert_eq!(check.path.as_deref(), Some("path/to/file"));
    assert!(check.strict);
}

// ---- globals may flank every level and keep order -------------------------

#[test]
fn globals_parse_before_and_after_every_command_token() {
    let (config, define, worker) = worker_ok(&[
        "tool",
        "--config",
        "root.toml",
        "serve",
        "--define",
        "a=1",
        "worker",
        "--port",
        "80",
        "--define",
        "b=2",
        "--config",
        "deep.toml",
    ]);
    assert_eq!(config, "deep.toml");
    assert_eq!(define, vec!["a=1", "b=2"]);
    assert_eq!(worker.port, 80);

    let (config, define, check) = check_ok(&[
        "tool", "check", "--config", "c.toml", "p", "--define", "k=v",
    ]);
    assert_eq!(config, "c.toml");
    assert_eq!(define, vec!["k=v"]);
    assert_eq!(check.path.as_deref(), Some("p"));

    // Attached forms work at every level as well.
    let (config, define, _) = worker_ok(&[
        "tool",
        "--config=c.toml",
        "serve",
        "--define=a=1",
        "worker",
        "--port=80",
    ]);
    assert_eq!(config, "c.toml");
    assert_eq!(define, vec!["a=1"]);
}

// ---- failures: port -------------------------------------------------------

#[test]
fn missing_port_fails_in_worker_context() {
    let (kind, msg) = fail_kind(&["tool", "serve", "worker"]);
    assert_eq!(kind, ErrorKind::MissingRequiredArgument);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
    assert!(!msg.contains("tool check"), "{msg}");

    // A global missing value must not masquerade as the missing port, and the
    // port is still reported as missing once such input is fixed.
    let (kind, msg) = fail_kind(&[
        "tool", "--define", "a=1", "serve", "--config", "c.toml", "worker",
    ]);
    assert_eq!(kind, ErrorKind::MissingRequiredArgument);
    assert!(msg.contains("--port"), "{msg}");
}

#[test]
fn out_of_range_port_is_an_invalid_value() {
    for argv in [
        &["tool", "serve", "worker", "--port", "0"][..],
        &["tool", "serve", "worker", "--port", "65536"][..],
        &["tool", "serve", "worker", "--port=99999999999999999999"][..],
    ] {
        let bad = argv[argv.len() - 1].trim_start_matches("--port=");
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains(bad), "{argv:?}: {msg}");
        assert!(msg.contains("--port"), "{argv:?}: {msg}");
    }
}

#[test]
fn non_numeric_port_is_an_invalid_value_not_an_unknown_token() {
    for argv in [
        &["tool", "serve", "worker", "--port", "abc"][..],
        &["tool", "serve", "worker", "--port", "1x"][..],
        &["tool", "serve", "worker", "--port", "+80"][..],
        &["tool", "serve", "worker", "--port=0x10"][..],
        &["tool", "serve", "worker", "--port="][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}: {msg}");
    }

    // A detached negative number is consumed by the option lexer as a flag and
    // therefore reported as the unknown token it actually is.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "-1"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("-1"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    // The attached form instead reaches the parser as the port's value.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port=-1"]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("'-1'"), "{msg}");
}

// ---- failures: protocol switch and duplicate arguments -------------------

#[test]
fn protocol_switches_conflict_in_worker_context() {
    for argv in [
        &["tool", "serve", "worker", "--port", "80", "--tcp", "--udp"][..],
        &["tool", "serve", "worker", "--tcp", "--udp", "--port", "80"][..],
        &[
            "tool", "--config", "c.toml", "serve", "worker", "--udp", "--port", "80", "--tcp",
        ][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::ArgumentConflict, "{argv:?}");
        assert!(msg.contains("--tcp"), "{argv:?}: {msg}");
        assert!(msg.contains("--udp"), "{argv:?}: {msg}");
        assert!(msg.contains("tool serve worker"), "{argv:?}: {msg}");
    }
}

#[test]
fn duplicate_non_repeatable_arguments_are_conflicts() {
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "80", "--port", "90"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "80", "--tcp", "--tcp"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--tcp"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    let (kind, msg) = fail_kind(&["tool", "check", "--strict", "--strict"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--strict"), "{msg}");
    assert!(msg.contains("tool check"), "{msg}");
}

#[test]
fn duplicate_root_config_is_reported_at_the_level_where_it_occurs() {
    let (kind, msg) = fail_kind(&["tool", "--config", "a", "--config", "b", "plug"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--config"), "{msg}");
    assert!(msg.contains("tool [OPTIONS] <COMMAND>"), "{msg}");

    // The same duplication reached after descending into worker names the
    // worker context; the conflict is never deferred to a plugin capture.
    let (kind, msg) = fail_kind(&[
        "tool", "serve", "worker", "--port", "80", "--config", "a", "--config", "b",
    ]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--config"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
}

// ---- failures: defines and missing values --------------------------------

#[test]
fn invalid_define_is_a_value_validation_error_at_every_active_level() {
    for argv in [
        &["tool", "--define", "nokey", "plug"][..],
        &["tool", "--define=nokey"][..],
        &[
            "tool", "serve", "worker", "--port", "80", "--define", "nokey",
        ][..],
        &["tool", "check", "--define", "nokey"][..],
        &["tool", "serve", "--define="][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(msg.contains("--define"), "{argv:?}: {msg}");
        // Each invalid value (including the empty one) is attributed by value.
        assert!(msg.contains("expected key=value"), "{argv:?}: {msg}");
    }
}

#[test]
fn invalid_define_after_an_external_name_is_plugin_data() {
    let (_, define, tokens) = root_external_ok(&["tool", "plug", "--define", "nokey"]);
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "--define", "nokey"]);

    let (_, define, tokens) = serve_external_ok(&["tool", "serve", "plug", "--define", "nokey"]);
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(tokens, vec!["plug", "--define", "nokey"]);
}

#[test]
fn missing_option_value_fails_on_the_option() {
    for argv in [
        &["tool", "--config"][..],
        &["tool", "--define"][..],
        &["tool", "serve", "worker", "--port", "80", "--config"][..],
        &["tool", "check", "--define"][..],
    ] {
        let needle = if argv.contains(&"--config") {
            "--config"
        } else {
            "--define"
        };
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::InvalidValue, "{argv:?}");
        assert!(msg.contains(needle), "{argv:?}: {msg}");
    }

    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port"]);
    assert_eq!(kind, ErrorKind::InvalidValue);
    assert!(msg.contains("--port"), "{msg}");
}

// ---- failures: unknown arguments and level isolation ----------------------

#[test]
fn unknown_option_fails_on_triggering_token_in_the_right_context() {
    let (kind, msg) = fail_kind(&["tool", "--bogus", "plug"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--bogus"), "{msg}");
    assert!(msg.contains("tool [OPTIONS] <COMMAND>"), "{msg}");

    let (kind, msg) = fail_kind(&["tool", "-x", "plug"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("-x"), "{msg}");

    let (kind, msg) = fail_kind(&["tool", "serve", "--bogus"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--bogus"), "{msg}");
    assert!(msg.contains("tool serve [OPTIONS] <COMMAND>"), "{msg}");

    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "80", "--bogus"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--bogus"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");
}

#[test]
fn worker_and_serve_arguments_are_unknown_to_check() {
    for argv in [
        &["tool", "check", "--port", "80"][..],
        &["tool", "check", "--tcp"][..],
        &["tool", "check", "--udp"][..],
    ] {
        let token = argv[2];
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::UnknownArgument, "{argv:?}");
        assert!(msg.contains(token), "{argv:?}: {msg}");
        assert!(msg.contains("tool check"), "{argv:?}: {msg}");
        assert!(!msg.contains("tool serve worker"), "{argv:?}: {msg}");
    }
}

#[test]
fn check_accepts_a_single_path_only() {
    let (kind, msg) = fail_kind(&["tool", "check", "a", "b"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("'b'"), "{msg}");
    assert!(msg.contains("tool check"), "{msg}");

    // Declared subcommand names are ordinary path values for check, not
    // dispatch tokens.
    let (_, _, check) = check_ok(&["tool", "check", "worker"]);
    assert_eq!(check.path.as_deref(), Some("worker"));
    assert!(!check.strict);
}

#[test]
fn worker_specific_flags_do_not_belong_to_serve_or_root() {
    // `serve` itself only dispatches; a worker flag with no plugin name is an
    // unknown serve token rather than an early worker parse.
    let (kind, msg) = fail_kind(&["tool", "serve", "--tcp"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--tcp"), "{msg}");
    assert!(msg.contains("tool serve [OPTIONS] <COMMAND>"), "{msg}");
}

// ---- help -----------------------------------------------------------------

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
    assert!(
        !help.contains("captured as a serve plugin"),
        "serve boundary leaked into root help:\n{help}"
    );
    assert!(!help.to_ascii_lowercase().contains("error"), "{help}");
}

#[test]
fn serve_help_succeeds_and_shows_only_serve_surface() {
    let help = help_text(&["tool", "serve", "--help"]);
    for needle in [
        "worker",
        "--config",
        "builtin.toml",
        "--define",
        "tool serve",
        "captured as a serve plugin",
    ] {
        assert!(
            help.contains(needle),
            "serve help missing {needle}:\n{help}"
        );
    }
    for leaked in ["--port", "--tcp", "--udp", "--strict", "[PATH]"] {
        assert!(
            !help.contains(leaked),
            "serve help unexpectedly contains {leaked}:\n{help}"
        );
    }
    assert!(
        !help.contains("captured as a root plugin"),
        "root boundary leaked into serve help:\n{help}"
    );
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
        "--define",
        "tool serve worker",
    ] {
        assert!(
            help.contains(needle),
            "worker help missing {needle}:\n{help}"
        );
    }
    for leaked in ["--strict", "root plugin", "serve plugin"] {
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
        "--strict",
        "PATH",
        "--config",
        "builtin.toml",
        "--define",
        "tool check",
    ] {
        assert!(
            help.contains(needle),
            "check help missing {needle}:\n{help}"
        );
    }
    for leaked in [
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

// ---- fixed-order isolation sequence ---------------------------------------

#[test]
fn fixed_order_sequence_two_externals_failure_worker_help_is_isolated() {
    // Clean baselines captured before anything else runs; the trailing help
    // requests must render byte-identical text after the intervening calls.
    let clean_root_help = help_text(&["tool", "--help"]);
    let clean_serve_help = help_text(&["tool", "serve", "--help"]);
    let clean_worker_help = help_text(&["tool", "serve", "worker", "--help"]);
    let clean_check_help = help_text(&["tool", "check", "--help"]);

    // 1. Root plugin success: root globals parse, plugin tokens stay verbatim.
    let (config, define, tokens) = root_external_ok(&[
        "tool",
        "--config",
        "custom.toml",
        "--define",
        "a=1",
        "rootplug",
        "--flag",
        "serve",
        "worker",
        "--",
        "",
    ]);
    assert_eq!(config, "custom.toml");
    assert_eq!(define, vec!["a=1"]);
    assert_eq!(
        tokens,
        vec!["rootplug", "--flag", "serve", "worker", "--", ""]
    );

    // 2. Serve plugin success in a fresh parse: defaults restored, the name is
    //    captured under serve and nothing leaks from the root plugin call.
    let (config, define, tokens) = serve_external_ok(&[
        "tool",
        "serve",
        "--define",
        "b=2",
        "serveplug",
        "--config",
        "x",
        "check",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, vec!["b=2"]);
    assert_eq!(tokens, vec!["serveplug", "--config", "x", "check"]);

    // 3. Invalid input fails categorically on the offending token and yields
    //    no readable partial match.
    let (kind, msg) = fail_kind(&["tool", "serve", "worker", "--port", "nope", "--tcp"]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("nope"), "{msg}");
    assert!(msg.contains("--port"), "{msg}");

    // 4. Worker success is unaffected by the three preceding parses: root
    //    defaults are back and no earlier tokens leak into trailing.
    let (config, define, worker) = worker_ok(&[
        "tool", "serve", "worker", "--port", "8080", "--udp", "--", "z", "--tcp", "--config",
    ]);
    assert_eq!(config, "builtin.toml");
    assert_eq!(define, Vec::<String>::new());
    assert_eq!(
        worker,
        WorkerArgs {
            port: 8080,
            tcp: false,
            udp: true,
            trailing: vec!["z".to_owned(), "--tcp".to_owned(), "--config".to_owned()],
        }
    );

    // 5. Every help request still succeeds, is identical to the clean
    //    baselines, and carries no trace of the failed call.
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
        assert!(!help.contains("nope"), "{argv:?}:\n{help}");
    }
}

// ---- moving globals changes nothing --------------------------------------

#[test]
fn moving_globals_does_not_change_success_structure_or_trailing_order() {
    let pairs: &[(&[&str], &[&str])] = &[
        // Root plugin: globals swapped among themselves before the name.
        (
            &[
                "tool", "--config", "c.toml", "--define", "a=1", "plug", "x", "--tcp",
            ],
            &[
                "tool", "--define", "a=1", "--config", "c.toml", "plug", "x", "--tcp",
            ],
        ),
        // Serve plugin: globals straddle the root/serve tokens before the name.
        (
            &[
                "tool", "--config", "c.toml", "serve", "--define", "a=1", "plug", "x",
            ],
            &[
                "tool", "serve", "--define", "a=1", "--config", "c.toml", "plug", "x",
            ],
        ),
        // Worker: globals flank every token while the defines keep order.
        (
            &[
                "tool", "--config", "c.toml", "serve", "--define", "a=1", "worker", "--port", "80",
                "--define", "b=2", "--", "x", "--port", "9",
            ],
            &[
                "tool", "serve", "worker", "--define", "a=1", "--port", "80", "--config", "c.toml",
                "--define", "b=2", "--", "x", "--port", "9",
            ],
        ),
        // Check: same options, root-side versus check-side placement.
        (
            &[
                "tool", "--config", "c.toml", "--define", "a=1", "check", "p", "--strict",
            ],
            &[
                "tool", "check", "p", "--strict", "--config", "c.toml", "--define", "a=1",
            ],
        ),
    ];
    for (left, right) in pairs {
        let a = parse(left).unwrap_or_else(|err| panic!("left failed {left:?}: {err}"));
        let b = parse(right).unwrap_or_else(|err| panic!("right failed {right:?}: {err}"));
        assert_eq!(a, b, "structure differs for {left:?} vs {right:?}");
    }
}

#[test]
fn moving_globals_does_not_change_error_kind_usage_or_trigger_token() {
    let pairs: &[(&[&str], &[&str])] = &[
        // Missing required port, globals anywhere around the command tokens.
        (
            &["tool", "serve", "worker"],
            &[
                "tool", "--config", "c.toml", "serve", "--define", "a=1", "worker",
            ],
        ),
        // Missing value for the port option.
        (
            &["tool", "serve", "worker", "--port"],
            &["tool", "--config", "c.toml", "serve", "worker", "--port"],
        ),
        // Invalid port value, globals on opposite sides.
        (
            &["tool", "serve", "worker", "--port", "0"],
            &[
                "tool", "--define", "a=1", "serve", "worker", "--port", "0", "--config", "c",
            ],
        ),
        // Protocol conflict, globals on opposite sides.
        (
            &["tool", "serve", "worker", "--tcp", "--udp", "--port", "80"],
            &[
                "tool", "serve", "--config", "c", "worker", "--port", "80", "--tcp", "--udp",
            ],
        ),
        // Worker option smuggled into check, with globals straddling.
        (
            &["tool", "check", "--port", "80"],
            &[
                "tool", "--config", "c.toml", "check", "--port", "80", "--define", "a=1",
            ],
        ),
        // Invalid define before versus after the command tokens.
        (
            &["tool", "--define", "bad", "serve", "worker", "--port", "80"],
            &["tool", "serve", "worker", "--port", "80", "--define", "bad"],
        ),
    ];
    for (left, right) in pairs {
        let (ka, ma) = fail_kind(left);
        let (kb, mb) = fail_kind(right);
        assert_eq!(ka, kb, "kinds differ for {left:?} vs {right:?}");
        assert_eq!(
            ma, mb,
            "rendered error differs for {left:?} vs {right:?}\n--- left ---\n{ma}\n--- right ---\n{mb}"
        );
    }
}

#[test]
fn moving_globals_does_not_change_rendered_help_or_its_ordering() {
    let pairs: &[(&[&str], &[&str])] = &[
        (
            &["tool", "--help"],
            &["tool", "--config", "c.toml", "--help"],
        ),
        (
            &["tool", "serve", "--help"],
            &[
                "tool", "--define", "a=1", "--config", "c.toml", "serve", "--help",
            ],
        ),
        (
            &["tool", "serve", "worker", "--help"],
            &[
                "tool", "serve", "--config", "c", "worker", "--define", "a=1", "--help",
            ],
        ),
        (
            &["tool", "check", "--help"],
            &[
                "tool", "--define", "a=1", "check", "--config", "c", "--help",
            ],
        ),
    ];
    for (left, right) in pairs {
        assert_eq!(
            help_text(left),
            help_text(right),
            "help differs for {left:?} vs {right:?}"
        );
    }
}
