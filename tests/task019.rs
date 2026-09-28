//! Standalone, executable acceptance target for the production builder path
//! of a layered plugin-dispatch CLI.
//!
//! This file never touches the derive facade: the command graph is built with
//! the public `clap::Command`/`Arg` builder and every result is observed
//! solely through the public production entry point
//! [`clap::Command::try_get_matches_from`] with argv0 included (`tool`, ...).
//! Nothing is read through test-only internals: success state comes from the
//! returned `ArgMatches`, failures from the public `clap::Error` (kind plus
//! rendered text) and help from the `DisplayHelp` error.
//!
//! Scenario under test:
//! * root command `tool` with a global `--config <path>` defaulting to
//!   `builtin.toml`, and a root-level repeatable `--define key=value` that
//!   accepts only exactly one non-empty key, one non-empty value and exactly
//!   one `=` separator, with no whitespace on either side of the pair;
//! * declared subcommands `serve` and `check`, plus a root external
//!   catch-all; `serve` declares `worker` and has its own external catch-all;
//!   every declared command also carries aliases (a visible and a hidden one):
//!   `serve` is reachable as `srv`/`serve-hidden`, `worker` as `wkr`/
//!   `worker-hidden` and `check` as `chk`/`check-hidden`;
//! * declared commands and every one of their aliases take precedence over
//!   external capture at the level that owns them: an alias dispatches to the
//!   declared command (validation, conflicts and help all report the canonical
//!   name), while the same spelling stays a plugin at a level that does not
//!   declare it; a standalone `--` still forces external capture even when the
//!   next token is a declared name or alias;
//! * `--define` is root-scoped rather than global: it parses only while the
//!   root command is active, keeps occurrence order, is rejected with the
//!   triggering token at any deeper level, and is never parsed once either
//!   external capture starts;
//! * `worker` requires exactly one `--port` built only from ASCII decimal
//!   digits in 1..=65535: empty strings, sign prefixes, hex, underscores,
//!   embedded whitespace and overflowing values are reported instead of being
//!   normalized, truncated or accepted;
//! * `--http` and `--https` are mutually exclusive; tokens after worker's own
//!   standalone `--` are trailing verbatim data in order, and without the
//!   separator a bare token is an unknown argument rather than trailing data;
//! * `check` takes only an optional path and `--strict` and never accepts
//!   worker-owned arguments;
//! * the global config parses before or after at any level while defines keep
//!   occurrence order at theirs, and once an external command name is fixed
//!   every following token belongs to that plugin verbatim;
//! * validation failures keep the distinguishable public error kinds for
//!   missing arguments, missing/invalid values, conflicts and unknown tokens,
//!   name the most specific command and the triggering token, and never leave
//!   a partial match readable;
//! * help for root, serve, worker and check succeeds even while required
//!   values are missing, shows only that level's surface, defaults,
//!   subcommands and external-capture boundary, and carries no stale error;
//! * a fixed sequence of external success, every invalid-define class, port
//!   failures, a valid worker after the failures and help requests is mutually
//!   isolated, and moving the global arguments around changes neither the
//!   success data, define order, trailing order, error usage nor help order.
#![cfg(feature = "help")]
#![cfg(feature = "usage")]

use clap::Arg;
use clap::ArgAction;
use clap::ArgMatches;
use clap::Command;
use clap::error::ErrorKind;
use clap::value_parser;

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

/// The production command graph, constructed through the public builder only.
///
/// A fresh `Command` is built for every parse: the builder is consumed by
/// `try_get_matches_from`, and rebuilding here also proves that one parse's
/// state can never leak into the next.
fn tool() -> Command {
    Command::new("tool")
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
        .subcommand_required(true)
        .arg_required_else_help(true)
        .allow_external_subcommands(true)
        .external_subcommand_value_parser(value_parser!(String))
        .subcommand(
            Command::new("serve")
                .visible_alias("srv")
                .alias("serve-hidden")
                .after_help("External plugins: any other <COMMAND> is captured as a serve plugin.")
                .subcommand_required(true)
                .arg_required_else_help(true)
                .allow_external_subcommands(true)
                .external_subcommand_value_parser(value_parser!(String))
                .subcommand(
                    Command::new("worker")
                        .visible_alias("wkr")
                        .alias("worker-hidden")
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
                        // Positional `Vec` slot, mirrored from the derive
                        // shape: Append + `num_args(1..)` + `last`, so the
                        // slot is reachable only through the standalone `--`
                        // and then consumes every remaining token verbatim.
                        .arg(
                            Arg::new("trailing")
                                .value_name("TRAILING")
                                .action(ArgAction::Append)
                                .num_args(1..)
                                .last(true),
                        ),
                ),
        )
        .subcommand(
            Command::new("check")
                .visible_alias("chk")
                .alias("check-hidden")
                .arg(Arg::new("path").value_name("PATH").action(ArgAction::Set))
                .arg(Arg::new("strict").long("strict").action(ArgAction::SetTrue)),
        )
}

fn parse(argv: &[&str]) -> Result<ArgMatches, clap::Error> {
    tool().try_get_matches_from(argv)
}

/// Success state observed on the `tool serve worker` path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct WorkerState {
    config: String,
    define: Vec<(String, String)>,
    port: u16,
    http: bool,
    https: bool,
    trailing: Vec<String>,
}

/// Success state observed on the `tool check` path.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CheckState {
    config: String,
    define: Vec<(String, String)>,
    path: Option<String>,
    strict: bool,
}

/// Success state observed on either external-capture path. The plugin command
/// name is the first token.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ExternalState {
    config: String,
    define: Vec<(String, String)>,
    tokens: Vec<String>,
}

fn read_config(matches: &ArgMatches) -> String {
    matches
        .get_one::<String>("config")
        .cloned()
        .expect("global --config always carries a value, default or explicit")
}

fn read_define(matches: &ArgMatches) -> Vec<(String, String)> {
    matches
        .get_many::<(String, String)>("define")
        .map(|values| values.cloned().collect())
        .unwrap_or_default()
}

fn external_tokens(name: &str, ext: &ArgMatches) -> Vec<String> {
    let mut tokens = vec![name.to_owned()];
    tokens.extend(
        ext.get_many::<String>("")
            .expect("external capture stores its tokens under the empty id")
            .cloned(),
    );
    tokens
}

/// An external capture is a subcommand whose own matches hold the captured
/// tokens under clap's empty id. That marker -- not the name -- distinguishes
/// it from a declared command, because a root-level `--` can force a token that
/// looks like a declared name (`serve`, `worker`) to be captured as a plugin.
fn is_external(matches: &ArgMatches) -> bool {
    matches.get_many::<String>("").is_some()
}

fn worker_ok(argv: &[&str]) -> WorkerState {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let (root_name, serve_matches) = matches
        .subcommand()
        .unwrap_or_else(|| panic!("expected a subcommand for {argv:?}, got root matches"));
    assert_eq!(root_name, "serve", "{argv:?}");
    let (name, worker_matches) = serve_matches
        .subcommand()
        .unwrap_or_else(|| panic!("expected `serve worker` for {argv:?}"));
    assert_eq!(name, "worker", "{argv:?}");
    let state = WorkerState {
        config: read_config(worker_matches),
        define: read_define(&matches),
        port: *worker_matches
            .get_one::<u16>("port")
            .unwrap_or_else(|| panic!("worker port missing for {argv:?}")),
        http: worker_matches.get_flag("http"),
        https: worker_matches.get_flag("https"),
        trailing: worker_matches
            .get_many::<String>("trailing")
            .map(|values| values.cloned().collect())
            .unwrap_or_default(),
    };
    eprintln!(
        "{argv:?} => worker: config={:?} define={:?} port={} http={} https={} trailing={:?}",
        state.config, state.define, state.port, state.http, state.https, state.trailing
    );
    state
}

fn check_ok(argv: &[&str]) -> CheckState {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let (name, check_matches) = matches
        .subcommand()
        .unwrap_or_else(|| panic!("expected a subcommand for {argv:?}"));
    assert_eq!(name, "check", "{argv:?}");
    let state = CheckState {
        config: read_config(check_matches),
        define: read_define(&matches),
        path: check_matches.get_one::<String>("path").cloned(),
        strict: check_matches.get_flag("strict"),
    };
    eprintln!(
        "{argv:?} => check: config={:?} define={:?} path={:?} strict={}",
        state.config, state.define, state.path, state.strict
    );
    state
}

fn root_external_ok(argv: &[&str]) -> ExternalState {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let (name, ext_matches) = matches
        .subcommand()
        .unwrap_or_else(|| panic!("expected an external subcommand for {argv:?}"));
    assert!(
        is_external(ext_matches),
        "declared command {name} dispatched for {argv:?}"
    );
    let state = ExternalState {
        config: read_config(&matches),
        define: read_define(&matches),
        tokens: external_tokens(name, ext_matches),
    };
    assert!(
        !state.tokens.is_empty(),
        "capture must include the name: {argv:?}"
    );
    eprintln!(
        "{argv:?} => root plugin: config={:?} define={:?} tokens={:?}",
        state.config, state.define, state.tokens
    );
    state
}

fn serve_external_ok(argv: &[&str]) -> ExternalState {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let (root_name, serve_matches) = matches
        .subcommand()
        .unwrap_or_else(|| panic!("expected a subcommand for {argv:?}"));
    assert_eq!(root_name, "serve", "{argv:?}");
    let (name, ext_matches) = serve_matches
        .subcommand()
        .unwrap_or_else(|| panic!("expected a serve external subcommand for {argv:?}"));
    assert!(
        is_external(ext_matches),
        "declared command {name} dispatched for {argv:?}"
    );
    let state = ExternalState {
        config: read_config(serve_matches),
        define: read_define(&matches),
        tokens: external_tokens(name, ext_matches),
    };
    assert!(
        !state.tokens.is_empty(),
        "capture must include the name: {argv:?}"
    );
    eprintln!(
        "{argv:?} => serve plugin: config={:?} define={:?} tokens={:?}",
        state.config, state.define, state.tokens
    );
    state
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

/// Semantic snapshot of a successful parse: only the business values the
/// caller can read back. Positional bookkeeping (the index at which an option
/// happened to appear) is intentionally excluded, since moving an option
/// changes that index without changing any success data.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Snapshot {
    Worker(WorkerState),
    Check(CheckState),
    RootExternal(ExternalState),
    ServeExternal(ExternalState),
}

fn snapshot(argv: &[&str]) -> Snapshot {
    let matches = parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
    let (name, sub) = matches.subcommand().expect("a subcommand is required");
    // Check the external marker first: a root-level `--` can force a plugin
    // whose name coincides with a declared command.
    if is_external(sub) {
        return Snapshot::RootExternal(ExternalState {
            config: read_config(&matches),
            define: read_define(&matches),
            tokens: external_tokens(name, sub),
        });
    }
    match name {
        "serve" => {
            let (serve_name, serve_sub) = sub.subcommand().expect("serve requires a subcommand");
            if is_external(serve_sub) {
                Snapshot::ServeExternal(ExternalState {
                    config: read_config(sub),
                    define: read_define(&matches),
                    tokens: external_tokens(serve_name, serve_sub),
                })
            } else {
                assert_eq!(serve_name, "worker", "{argv:?}");
                Snapshot::Worker(WorkerState {
                    config: read_config(serve_sub),
                    define: read_define(&matches),
                    port: *serve_sub.get_one::<u16>("port").expect("worker port"),
                    http: serve_sub.get_flag("http"),
                    https: serve_sub.get_flag("https"),
                    trailing: serve_sub
                        .get_many::<String>("trailing")
                        .map(|values| values.cloned().collect())
                        .unwrap_or_default(),
                })
            }
        }
        "check" => Snapshot::Check(CheckState {
            config: read_config(sub),
            define: read_define(&matches),
            path: sub.get_one::<String>("path").cloned(),
            strict: sub.get_flag("strict"),
        }),
        other => panic!("unexpected declared subcommand {other} for {argv:?}"),
    }
}

// ---- dispatch: declared commands take precedence over external capture ----

#[test]
fn declared_commands_are_not_swallowed_by_external_capture() {
    // Declared names dispatch to the built-in commands at both levels.
    let matches = parse(&["tool", "check"]).unwrap();
    assert_eq!(matches.subcommand().unwrap().0, "check");
    let worker = worker_ok(&["tool", "serve", "worker", "--port", "8080"]);
    assert_eq!(worker.port, 8080);

    // Prefixes and look-alikes stay external.
    assert_eq!(
        root_external_ok(&["tool", "ser", "x"]).tokens,
        vec!["ser", "x"]
    );
    assert_eq!(
        serve_external_ok(&["tool", "serve", "work", "x"]).tokens,
        vec!["work", "x"]
    );
    // `check` is declared at the root only, so under `serve` it is a plugin.
    assert_eq!(
        serve_external_ok(&["tool", "serve", "check"]).tokens,
        vec!["check"]
    );

    // A bare `worker` at the root is a root plugin, never the nested command.
    let ext = root_external_ok(&["tool", "worker", "--port", "8080"]);
    assert_eq!(ext.tokens, vec!["worker", "--port", "8080"]);
}

// ---- dispatch: declared aliases take precedence over external capture ------

#[test]
fn aliases_dispatch_to_their_declared_commands_under_canonical_names() {
    // Root-level aliases resolve to the canonical subcommand name.
    assert_eq!(
        parse(&["tool", "chk", "p"])
            .unwrap()
            .subcommand()
            .unwrap()
            .0,
        "check"
    );
    assert_eq!(
        parse(&["tool", "check-hidden", "p"])
            .unwrap()
            .subcommand()
            .unwrap()
            .0,
        "check"
    );

    // Serve aliases resolve under the canonical `serve` name: a plugin reached
    // through an alias is reported as `serve`'s external subcommand.
    for serve in ["srv", "serve-hidden"] {
        let ext = serve_external_ok(&["tool", serve, "plug"]);
        assert_eq!(ext.tokens, vec!["plug"]);
    }

    // The worker alias resolves through either spelling of `serve`, and the
    // reported chain always uses the canonical names `serve`/`worker`.
    for serve in ["serve", "srv", "serve-hidden"] {
        for worker in ["worker", "wkr", "worker-hidden"] {
            let argv = ["tool", serve, worker, "--port", "8080"];
            let state = worker_ok(&argv);
            assert_eq!(state.port, 8080, "{argv:?}");
        }
    }

    // The check alias accepts check's own surface.
    let check = check_ok(&["tool", "chk", "path", "--strict"]);
    assert_eq!(check.path.as_deref(), Some("path"));
    assert!(check.strict);
}

#[test]
fn aliases_take_precedence_over_external_capture_at_their_own_level() {
    // A token that is an alias at this level dispatches; it is never captured
    // as a plugin even when followed by tokens that look like plugin data.
    for argv in [
        &["tool", "srv", "wkr", "--port", "80"][..],
        &["tool", "serve-hidden", "worker-hidden", "--port", "80"][..],
        &["tool", "chk", "path", "--strict"][..],
    ] {
        let matches =
            parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"));
        let (root_name, _) = matches.subcommand().unwrap();
        assert!(
            root_name == "serve" || root_name == "check",
            "{argv:?} dispatched to {root_name}"
        );
    }

    // The same name spelled through aliases but rejected at the declared
    // command is the declared command's own failure, not a plugin capture.
    let (kind, msg) = fail_kind(&["tool", "srv", "wkr"]);
    assert_eq!(kind, ErrorKind::MissingRequiredArgument);
    assert!(msg.contains("tool serve worker"), "{msg}");

    // A plugin whose name merely starts like an alias is still external
    // (neither prefix inference nor prefix capture applies).
    assert_eq!(
        root_external_ok(&["tool", "sr", "x"]).tokens,
        vec!["sr", "x"]
    );
    assert_eq!(
        serve_external_ok(&["tool", "srv", "wk", "x"]).tokens,
        vec!["wk", "x"]
    );
}

#[test]
fn an_alias_at_one_level_is_a_plugin_at_a_level_that_does_not_declare_it() {
    // `wkr`/`worker-hidden` name worker only under serve; at the root they are
    // ordinary root plugin names and their tokens stay verbatim.
    assert_eq!(
        root_external_ok(&["tool", "wkr", "--port", "80"]).tokens,
        vec!["wkr", "--port", "80"]
    );
    assert_eq!(
        root_external_ok(&["tool", "worker-hidden", "x"]).tokens,
        vec!["worker-hidden", "x"]
    );

    // `chk`/`check-hidden` name check only at the root; under serve they are
    // serve-level plugins.
    assert_eq!(
        serve_external_ok(&["tool", "srv", "chk"]).tokens,
        vec!["chk"]
    );
    assert_eq!(
        serve_external_ok(&["tool", "serve", "check-hidden"]).tokens,
        vec!["check-hidden"]
    );

    // Serve's own alias spelling is not reserved beneath it.
    assert_eq!(
        serve_external_ok(&["tool", "serve", "srv"]).tokens,
        vec!["srv"]
    );
}

#[test]
fn a_standalone_separator_forces_external_capture_even_for_an_alias() {
    // After a root `--`, a declared alias is a plugin name, not the command.
    assert_eq!(
        root_external_ok(&["tool", "--", "srv", "x"]).tokens,
        vec!["srv", "x"]
    );
    assert_eq!(root_external_ok(&["tool", "--", "chk"]).tokens, vec!["chk"]);
    // After serve's `--`, the worker alias is a serve plugin name.
    assert_eq!(
        serve_external_ok(&["tool", "srv", "--", "wkr", "--port", "1"]).tokens,
        vec!["wkr", "--port", "1"]
    );
    // But the separator at the level above the alias still selects the alias
    // for that level: `srv -- wkr` reaches serve, whose `--` then captures.
    let matches = parse(&["tool", "srv", "--", "wkr"]).unwrap();
    let (root_name, serve_matches) = matches.subcommand().unwrap();
    assert_eq!(root_name, "serve");
    let (captured, ext_matches) = serve_matches.subcommand().unwrap();
    assert_eq!(captured, "wkr");
    assert!(is_external(ext_matches));
}

#[test]
fn wrong_level_and_plugin_data_rules_hold_through_aliases() {
    // A root-scoped `--define` beneath a serve reached through its alias is an
    // unknown token attributed to the canonical `tool serve`, not the alias.
    let (kind, msg) = fail_kind(&["tool", "srv", "--define", "a=1", "plug"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--define"), "{msg}");
    assert!(msg.contains("tool serve"), "{msg}");
    assert!(!msg.contains("srv"), "alias leaked into usage: {msg}");

    // Beneath worker reached via both aliases, the same holds.
    let (kind, msg) = fail_kind(&["tool", "srv", "wkr", "--port", "80", "--define", "a=1"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("tool serve worker"), "{msg}");

    // Once the serve plugin name is fixed (after an alias-selected serve), a
    // `--define`-shaped token is plugin data, never re-parsed.
    assert_eq!(
        serve_external_ok(&["tool", "srv", "plug", "--define", "=v"]).tokens,
        vec!["plug", "--define", "=v"]
    );
}

#[test]
fn missing_subcommand_shows_that_levels_help_without_dispatching() {
    let (kind, _) = fail_kind(&["tool"]);
    assert_eq!(kind, ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand);
    let (kind, msg) = fail_kind(&["tool", "serve"]);
    assert_eq!(kind, ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand);
    assert!(msg.contains("tool serve"), "{msg}");
    assert!(msg.contains("worker"), "{msg}");
}

// ---- define success: strict pairs in occurrence order ---------------------

#[test]
fn defines_keep_occurrence_order_and_split_into_pairs() {
    let state = worker_ok(&[
        "tool",
        "--define",
        "a=1",
        "--define",
        "b=two",
        "--define=c=3",
        "--config",
        "custom.toml",
        "serve",
        "worker",
        "--port",
        "8080",
    ]);
    assert_eq!(state.config, "custom.toml");
    assert_eq!(
        state.define,
        vec![
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "two".to_owned()),
            ("c".to_owned(), "3".to_owned()),
        ]
    );
    assert_eq!(state.port, 8080);
}

#[test]
fn define_pairs_keep_inner_punctuation_verbatim() {
    // Punctuation that is neither `=` nor whitespace is part of the data and
    // is never trimmed or reinterpreted.
    let state = worker_ok(&[
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
        state.define,
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
    let ext = root_external_ok(&["tool", "--define", "k=v", "plug", "--define", "x=y"]);
    assert_eq!(ext.define, vec![("k".to_owned(), "v".to_owned())]);
    assert_eq!(ext.tokens, vec!["plug", "--define", "x=y"]);

    // Serve capture: a define between `serve` and the plugin name is a
    // serve-level token and is covered by the wrong-level failures; once the
    // serve plugin name is selected, define-like tokens are plugin data.
    let ext = serve_external_ok(&[
        "tool", "--define", "k=v", "serve", "plug", "--define", "x=y",
    ]);
    assert_eq!(ext.define, vec![("k".to_owned(), "v".to_owned())]);
    assert_eq!(ext.tokens, vec!["plug", "--define", "x=y"]);
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
                &format!("--define={raw}"),
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
    assert_eq!(
        worker_ok(&["tool", "serve", "worker", "--port", "1"]).port,
        1
    );
    assert_eq!(
        worker_ok(&["tool", "serve", "worker", "--port=65535"]).port,
        65535
    );
    // Leading zeroes are still plain decimal digits.
    assert_eq!(
        worker_ok(&["tool", "serve", "worker", "--port", "080"]).port,
        80
    );
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
fn missing_duplicate_and_conflicting_arguments_keep_distinct_kinds() {
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

    // Neither protocol switch alone is a conflict.
    let worker = worker_ok(&["tool", "serve", "worker", "--port", "80", "--https"]);
    assert!(!worker.http);
    assert!(worker.https);

    // The four public categories stay distinguishable from each other.
    let (invalid, _) = fail_kind(&["tool", "serve", "worker", "--port", "0"]);
    assert_eq!(invalid, ErrorKind::ValueValidation);
    assert_ne!(
        ErrorKind::MissingRequiredArgument,
        ErrorKind::ArgumentConflict
    );
    assert_ne!(
        ErrorKind::MissingRequiredArgument,
        ErrorKind::UnknownArgument
    );
    assert_ne!(ErrorKind::MissingRequiredArgument, invalid);
    assert_ne!(ErrorKind::ArgumentConflict, invalid);
    assert_ne!(ErrorKind::UnknownArgument, invalid);
}

#[test]
fn errors_reached_through_aliases_keep_kind_token_and_canonical_command() {
    // Missing required port through both visible and hidden aliases.
    for argv in [
        &["tool", "srv", "wkr"][..],
        &["tool", "serve-hidden", "worker-hidden"][..],
    ] {
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::MissingRequiredArgument, "{argv:?}");
        assert!(msg.contains("--port"), "{argv:?}: {msg}");
        assert!(msg.contains("tool serve worker"), "{argv:?}: {msg}");
        assert!(!msg.contains("srv"), "{argv:?}: {msg}");
        assert!(!msg.contains("wkr"), "{argv:?}: {msg}");
    }

    // Invalid value through aliases: the offending value and its option are
    // named verbatim. Invalid-value errors intentionally carry no usage block,
    // so only the option and value (not a usage command) are asserted.
    let (kind, msg) = fail_kind(&["tool", "srv", "wkr", "--port", "0"]);
    assert_eq!(kind, ErrorKind::ValueValidation);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("'0'"), "{msg}");

    // Duplicate port through aliases is still the conflict kind.
    let (kind, msg) = fail_kind(&["tool", "srv", "wkr", "--port", "80", "--port", "90"]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    // Protocol conflict through aliases.
    let (kind, msg) = fail_kind(&[
        "tool",
        "serve-hidden",
        "worker-hidden",
        "--port",
        "80",
        "--http",
        "--https",
    ]);
    assert_eq!(kind, ErrorKind::ArgumentConflict);
    assert!(msg.contains("--http"), "{msg}");
    assert!(msg.contains("--https"), "{msg}");
    assert!(msg.contains("tool serve worker"), "{msg}");

    // A worker-only option smuggled into check reached via its alias is an
    // unknown token attributed to the canonical `tool check`.
    let (kind, msg) = fail_kind(&["tool", "chk", "--port", "80"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("tool check"), "{msg}");
    assert!(!msg.contains("tool serve worker"), "{msg}");
    assert!(!msg.contains("chk"), "alias leaked into usage: {msg}");

    // Rendered errors are identical between the alias and canonical spelling,
    // because the alias is normalized away before any usage is produced.
    let (_, via_canonical) = fail_kind(&[
        "tool", "serve", "worker", "--port", "0", "--http", "--https",
    ]);
    let (_, via_alias) = fail_kind(&["tool", "srv", "wkr", "--port", "0", "--http", "--https"]);
    assert_eq!(via_canonical, via_alias);
}

#[test]
fn check_and_plugin_paths_do_not_inherit_the_port_constraint() {
    // Numeric-looking paths are plain data for `check`.
    let check = check_ok(&["tool", "check", "65536"]);
    assert_eq!(check.path.as_deref(), Some("65536"));

    // The port and protocol options are unknown to `check`, not reinterpreted.
    for argv in [
        &["tool", "check", "--port", "80"][..],
        &["tool", "check", "--http"][..],
        &["tool", "check", "--https"][..],
    ] {
        let token = argv[2];
        let (kind, msg) = fail_kind(argv);
        assert_eq!(kind, ErrorKind::UnknownArgument, "{argv:?}");
        assert!(msg.contains(token), "{argv:?}: {msg}");
        assert!(msg.contains("tool check"), "{argv:?}: {msg}");
        assert!(!msg.contains("tool serve worker"), "{argv:?}: {msg}");
    }

    // Plugin captures pass port-shaped tokens through verbatim.
    let ext = root_external_ok(&["tool", "plug", "--port", "0", "--define", "bad"]);
    assert_eq!(ext.define, Vec::<(String, String)>::new());
    assert_eq!(ext.tokens, vec!["plug", "--port", "0", "--define", "bad"]);

    let ext = serve_external_ok(&["tool", "serve", "plug", "--port", "abc"]);
    assert_eq!(ext.define, Vec::<(String, String)>::new());
    assert_eq!(ext.tokens, vec!["plug", "--port", "abc"]);
}

#[test]
fn check_accepts_one_optional_path_and_strict_only() {
    let empty = check_ok(&["tool", "check"]);
    assert_eq!(empty.config, "builtin.toml");
    assert_eq!(empty.path, None);
    assert!(!empty.strict);

    let strict = check_ok(&["tool", "check", "--strict", "path/to/file"]);
    assert_eq!(strict.path.as_deref(), Some("path/to/file"));
    assert!(strict.strict);

    // Declared subcommand names are ordinary path values for check.
    let named = check_ok(&["tool", "check", "worker"]);
    assert_eq!(named.path.as_deref(), Some("worker"));

    // A second positional has no slot and is rejected, never consumed.
    let (kind, msg) = fail_kind(&["tool", "check", "a", "b"]);
    assert_eq!(kind, ErrorKind::UnknownArgument);
    assert!(msg.contains("'b'"), "{msg}");
    assert!(msg.contains("tool check"), "{msg}");
}

// ---- worker trailing arguments: only after a standalone `--` --------------

#[test]
fn worker_trailing_after_separator_is_verbatim_and_never_redispatches() {
    let state = worker_ok(&[
        "tool", "serve", "worker", "--port", "8080", "--http", "--", "plug", "--http", "--https",
        "--port", "9090", "worker", "serve", "--", "",
    ]);
    assert_eq!(state.port, 8080);
    assert!(state.http);
    assert!(!state.https);
    assert_eq!(
        state.trailing,
        vec![
            "plug", "--http", "--https", "--port", "9090", "worker", "serve", "--", "",
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

// ---- external command success ----------------------------------------------

#[test]
fn external_captures_keep_every_token_after_the_name_verbatim() {
    let ext = root_external_ok(&[
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
    assert_eq!(ext.config, "custom.toml");
    assert_eq!(ext.define, vec![("a".to_owned(), "1".to_owned())]);
    assert_eq!(
        ext.tokens,
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

    let ext = serve_external_ok(&[
        "tool", "--define", "b=2", "serve", "plug", "worker", "--port", "0", "check",
    ]);
    assert_eq!(ext.config, "builtin.toml");
    assert_eq!(ext.define, vec![("b".to_owned(), "2".to_owned())]);
    assert_eq!(ext.tokens, vec!["plug", "worker", "--port", "0", "check"]);
}

#[test]
fn help_and_separator_tokens_after_an_external_name_are_plugin_data() {
    // `--help` after the name is captured, not handled by clap.
    let ext = root_external_ok(&["tool", "plug", "--help"]);
    assert_eq!(ext.tokens, vec!["plug", "--help"]);

    // A root-level separator forces the next token to be the plugin name, and
    // declared names after it are plugin names rather than commands.
    let ext = root_external_ok(&["tool", "--", "serve", "x"]);
    assert_eq!(ext.tokens, vec!["serve", "x"]);
    let ext = root_external_ok(&["tool", "--", "", "rest"]);
    assert_eq!(ext.tokens, vec!["", "rest"]);

    // The same separator semantics hold under `serve`.
    let ext = serve_external_ok(&["tool", "serve", "--", "worker", "--port", "1"]);
    assert_eq!(ext.tokens, vec!["worker", "--port", "1"]);
    let ext = serve_external_ok(&["tool", "serve", "--", "", "x"]);
    assert_eq!(ext.tokens, vec!["", "x"]);
}

// ---- global config position compatibility ---------------------------------

#[test]
fn global_config_parses_at_any_position_without_changing_the_structure() {
    fn parsed(argv: &[&str]) -> WorkerState {
        worker_ok(argv)
    }
    let before = parsed(&[
        "tool", "--config", "c.toml", "serve", "worker", "--port", "80",
    ]);
    let after = parsed(&[
        "tool", "serve", "worker", "--port", "80", "--config", "c.toml",
    ]);
    assert_eq!(before, after);

    let check_before = check_ok(&["tool", "--config", "c.toml", "check", "p", "--strict"]);
    let check_after = check_ok(&["tool", "check", "p", "--strict", "--config", "c.toml"]);
    assert_eq!(check_before, check_after);

    // Explicit config given next to a deep command is visible there.
    let deep = worker_ok(&[
        "tool", "serve", "--config", "s.toml", "worker", "--port", "80",
    ]);
    assert_eq!(deep.config, "s.toml");

    // After an external name, `--config` is plugin data, not a global.
    let ext = root_external_ok(&["tool", "--config", "c.toml", "plug", "--config", "x"]);
    assert_eq!(ext.config, "c.toml");
    assert_eq!(ext.tokens, vec!["plug", "--config", "x"]);

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
    for leaked in [
        "--port", "--http", "--https", "--strict", "TRAILING", "worker",
    ] {
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

#[test]
fn help_reached_through_an_alias_is_identical_and_shows_visible_aliases_only() {
    // Help renders the canonical command regardless of how it was reached:
    // the alias never appears in the usage line and the text is byte-identical
    // to the canonical spelling.
    for (canonical, aliased) in [
        (
            &["tool", "serve", "--help"][..],
            &["tool", "srv", "--help"][..],
        ),
        (
            &["tool", "serve", "worker", "--help"][..],
            &["tool", "srv", "wkr", "--help"][..],
        ),
        (
            &["tool", "check", "--help"][..],
            &["tool", "chk", "--help"][..],
        ),
        // Hidden aliases reach help too, with the same rendering.
        (
            &["tool", "serve", "--help"][..],
            &["tool", "serve-hidden", "--help"][..],
        ),
        (
            &["tool", "serve", "worker", "--help"][..],
            &["tool", "serve-hidden", "worker-hidden", "--help"][..],
        ),
        (
            &["tool", "check", "--help"][..],
            &["tool", "check-hidden", "--help"][..],
        ),
    ] {
        let (canonical_text, aliased_text) = (help_text(canonical), help_text(aliased));
        assert_eq!(
            canonical_text, aliased_text,
            "help differs for {canonical:?} vs {aliased:?}"
        );
    }

    // Root help lists the visible aliases alongside their commands but never
    // the hidden spellings, and shows serve's alias at the root but not
    // worker's nested alias.
    let root = help_text(&["tool", "--help"]);
    assert!(root.contains("srv"), "{root}");
    assert!(root.contains("chk"), "{root}");
    assert!(!root.contains("serve-hidden"), "{root}");
    assert!(!root.contains("check-hidden"), "{root}");
    assert!(
        !root.contains("wkr"),
        "nested alias leaked into root help:\n{root}"
    );

    // Serve help lists worker's visible alias but none of the hidden ones.
    let serve = help_text(&["tool", "serve", "--help"]);
    assert!(serve.contains("wkr"), "{serve}");
    assert!(!serve.contains("worker-hidden"), "{serve}");
    assert!(
        !serve.contains("srv"),
        "the command's own alias leaked in:\n{serve}"
    );
}

#[test]
fn missing_subcommand_help_through_an_alias_names_the_canonical_command() {
    let (kind, msg) = fail_kind(&["tool", "srv"]);
    assert_eq!(kind, ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand);
    assert!(msg.contains("tool serve"), "{msg}");
    assert!(msg.contains("worker"), "{msg}");
    assert!(msg.contains("wkr"), "visible alias should be listed: {msg}");
    assert!(
        !msg.contains("srv"),
        "the serve alias leaked into serve help: {msg}"
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

    // 1. External success at both levels: plugin tokens stay verbatim.
    let ext = root_external_ok(&[
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
    assert_eq!(ext.config, "custom.toml");
    assert_eq!(ext.define, vec![("a".to_owned(), "1".to_owned())]);
    assert_eq!(
        ext.tokens,
        vec!["rootplug", "--flag", "serve", "worker", "--", ""]
    );

    let ext = serve_external_ok(&["tool", "serve", "serveplug", "--config", "x", "check"]);
    assert_eq!(ext.config, "builtin.toml");
    assert_eq!(ext.define, Vec::<(String, String)>::new());
    assert_eq!(ext.tokens, vec!["serveplug", "--config", "x", "check"]);

    // 1b. The same external capture works when serve is reached by an alias;
    //     the alias normalizes to the declared level before capture begins.
    let via_alias = serve_external_ok(&["tool", "srv", "serveplug", "--config", "x", "check"]);
    assert_eq!(via_alias.config, ext.config);
    assert_eq!(via_alias.define, ext.define);
    assert_eq!(via_alias.tokens, ext.tokens);
    // An alias spelling beneath a level that does not own it stays a plugin.
    assert_eq!(
        root_external_ok(&["tool", "wkr", "--port", "0"]).tokens,
        vec!["wkr", "--port", "0"]
    );

    // 2. Every invalid-define class fails on its own token.
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

    // 4. A valid worker parse after the failures sees only its own input.
    let worker = worker_ok(&[
        "tool", "serve", "worker", "--port", "9090", "--https", "--", "z", "--define",
    ]);
    assert_eq!(worker.config, "builtin.toml");
    assert_eq!(worker.define, Vec::<(String, String)>::new());
    assert_eq!(
        worker,
        WorkerState {
            config: "builtin.toml".to_owned(),
            define: vec![],
            port: 9090,
            http: false,
            https: true,
            trailing: vec!["z".to_owned(), "--define".to_owned()],
        }
    );

    // 4b. The same valid parse reached entirely through aliases is isolated
    //     from the failures and reads exactly its own data.
    let aliased = worker_ok(&[
        "tool", "srv", "wkr", "--port", "9090", "--https", "--", "z", "--define",
    ]);
    assert_eq!(aliased, worker);

    // 5. Every help request still succeeds while required values are missing,
    //    is identical to the clean baselines, and carries no stale error.
    for (argv, clean) in [
        (&["tool", "--help"][..], &clean_root_help),
        (&["tool", "serve", "--help"][..], &clean_serve_help),
        (
            &["tool", "serve", "worker", "--help"][..],
            &clean_worker_help,
        ),
        (&["tool", "check", "--help"][..], &clean_check_help),
        // Alias-reached help is the same rendering as the canonical baselines.
        (&["tool", "srv", "--help"][..], &clean_serve_help),
        (&["tool", "srv", "wkr", "--help"][..], &clean_worker_help),
        (&["tool", "chk", "--help"][..], &clean_check_help),
    ] {
        let help = help_text(argv);
        assert_eq!(&help, clean, "{argv:?} help changed across the sequence");
        assert!(!help.contains("novalue"), "{argv:?}:\n{help}");
        assert!(!help.contains("0x10"), "{argv:?}:\n{help}");
    }
}

// ---- moving the global arguments changes nothing ----------------------------

#[test]
fn moving_globals_does_not_change_success_state_or_ordering() {
    let pairs: &[(&[&str], &[&str])] = &[
        // Worker: config on opposite sides of the command tokens.
        (
            &[
                "tool", "--config", "c.toml", "serve", "worker", "--port", "80",
            ],
            &[
                "tool", "serve", "worker", "--port", "80", "--config", "c.toml",
            ],
        ),
        // Check: same options, root-side versus check-side placement.
        (
            &["tool", "--config", "c.toml", "check", "p", "--strict"],
            &["tool", "check", "p", "--strict", "--config", "c.toml"],
        ),
        // Root plugin: config and define swapped before the name; define order
        // is preserved.
        (
            &[
                "tool", "--config", "c.toml", "--define", "a=1", "--define", "b=2", "plug", "x",
                "--http",
            ],
            &[
                "tool", "--define", "a=1", "--config", "c.toml", "--define", "b=2", "plug", "x",
                "--http",
            ],
        ),
        // Serve plugin: globals straddle the root/serve tokens before the name.
        (
            &["tool", "--config", "c.toml", "serve", "plug", "x"],
            &["tool", "serve", "--config", "c.toml", "plug", "x"],
        ),
        // Worker: the define order and the trailing order are position-proof.
        (
            &[
                "tool", "--config", "c.toml", "--define", "a=1", "serve", "worker", "--port", "80",
                "--", "x", "--port", "9",
            ],
            &[
                "tool", "--define", "a=1", "serve", "--config", "c.toml", "worker", "--port", "80",
                "--", "x", "--port", "9",
            ],
        ),
        // The whole chain reached through aliases yields the identical success
        // snapshot, with globals straddling the alias tokens.
        (
            &[
                "tool", "--config", "c.toml", "--define", "a=1", "srv", "wkr", "--port", "80",
                "--", "x", "--port", "9",
            ],
            &[
                "tool", "--define", "a=1", "srv", "--config", "c.toml", "wkr", "--port", "80",
                "--", "x", "--port", "9",
            ],
        ),
        // Check reached through its alias, globals on opposite sides.
        (
            &["tool", "--config", "c.toml", "chk", "p", "--strict"],
            &["tool", "chk", "p", "--strict", "--config", "c.toml"],
        ),
    ];
    for (left, right) in pairs {
        assert_eq!(
            snapshot(left),
            snapshot(right),
            "success state differs for {left:?} vs {right:?}"
        );
    }
}

#[test]
fn moving_globals_does_not_change_error_kind_usage_or_trigger_token() {
    let pairs: &[(&[&str], &[&str])] = &[
        // Missing required port, globals anywhere around the command tokens.
        (
            &["tool", "serve", "worker"],
            &["tool", "--config", "c.toml", "serve", "worker"],
        ),
        // Missing value for the port option.
        (
            &["tool", "serve", "worker", "--port"],
            &["tool", "--config", "c.toml", "serve", "worker", "--port"],
        ),
        // Missing value for the root define option.
        (
            &["tool", "--define"],
            &["tool", "--config", "c.toml", "--define"],
        ),
        // Invalid port value, globals on opposite sides.
        (
            &["tool", "serve", "worker", "--port", "0"],
            &[
                "tool", "serve", "worker", "--port", "0", "--config", "c.toml",
            ],
        ),
        // Protocol conflict, globals on opposite sides.
        (
            &[
                "tool", "serve", "worker", "--http", "--https", "--port", "80",
            ],
            &[
                "tool", "--config", "c", "serve", "worker", "--port", "80", "--http", "--https",
            ],
        ),
        // Worker option smuggled into check, with globals straddling.
        (
            &["tool", "check", "--port", "80"],
            &["tool", "--config", "c.toml", "check", "--port", "80"],
        ),
        // Invalid define before the subcommand token, config moved around it.
        (
            &["tool", "--define", "bad", "serve", "worker", "--port", "80"],
            &[
                "tool", "--config", "c.toml", "--define", "bad", "serve", "worker", "--port", "80",
            ],
        ),
        // Missing required port: alias chain versus canonical chain render the
        // same usage, and globals straddling the aliases change nothing.
        (&["tool", "serve", "worker"], &["tool", "srv", "wkr"]),
        (
            &["tool", "serve", "worker"],
            &[
                "tool",
                "--config",
                "c.toml",
                "serve-hidden",
                "worker-hidden",
            ],
        ),
        // Invalid value reached through aliases renders the canonical error.
        (
            &["tool", "serve", "worker", "--port", "0"],
            &["tool", "srv", "wkr", "--port", "0"],
        ),
        // Check-level unknown argument, reached through its alias.
        (
            &["tool", "check", "--http"],
            &["tool", "--config", "c.toml", "chk", "--http"],
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
            &["tool", "--config", "c.toml", "serve", "--help"],
        ),
        (
            &["tool", "serve", "worker", "--help"],
            &["tool", "serve", "--config", "c", "worker", "--help"],
        ),
        (
            &["tool", "check", "--help"],
            &["tool", "--config", "c", "check", "--help"],
        ),
        // Help reached through aliases renders the canonical ordering.
        (&["tool", "serve", "--help"], &["tool", "srv", "--help"]),
        (
            &["tool", "serve", "worker", "--help"],
            &["tool", "srv", "wkr", "--help"],
        ),
        (&["tool", "check", "--help"], &["tool", "chk", "--help"]),
        // Hidden aliases reach the same help, globals straddling the tokens.
        (
            &["tool", "serve", "worker", "--help"],
            &[
                "tool",
                "serve-hidden",
                "--config",
                "c",
                "worker-hidden",
                "--help",
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
