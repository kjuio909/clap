//! Standalone, executable regression target for layered subcommand and plugin
//! dispatch.
//!
//! This file is a self-contained test target, in the style of `task015.rs`,
//! that depends on no private test module. It defines its own derived CLI and
//! observes every result solely through the public `clap::Parser` API via
//! `Cli::try_parse_from` (argv0 included): successful structures, typed errors,
//! and rendered help text.
//!
//! Scenario under test:
//! * root command `tool` with a default config path and repeatable
//!   `--define` items, both global, declaring `serve` and `check` while keeping
//!   a root-level external-subcommand catch-all;
//! * `serve` declares `worker` and hands every other name to its own external
//!   plugin catch-all;
//! * `worker` requires a unique `--port` that is a decimal number in
//!   `1..=65535`, has mutually exclusive `--tcp`/`--udp` switches, and stores
//!   trailing arguments after its own `--` verbatim;
//! * `check` accepts only one optional path and `--strict`, inheriting none of
//!   `serve`/`worker`'s arguments;
//! * global config/definitions may appear before, between, or after the
//!   command tokens, with the explicit config overriding the default and
//!   definitions kept in command-line order;
//! * declared subcommands win over external capture, but once an external name
//!   is chosen every following token belongs to that plugin;
//! * a root-level `--` names an external command, while `worker`'s `--` only
//!   produces trailing arguments;
//! * validation failures name the most specific command and the triggering
//!   token and distinguish missing, invalid-value, conflict, and unknown-token
//!   cases, leaving nothing partially populated that a later parse could read;
//! * help at every level succeeds while required values are missing;
//! * a fixed sequence of root-plugin success, serve-plugin success, invalid
//!   input, worker success, and help is mutually isolated, and merely moving
//!   global arguments around changes neither the parsed structure, trailing
//!   order, error usage, nor the order of definitions in help.
#![cfg(feature = "derive")]
#![cfg(feature = "help")]
#![cfg(feature = "usage")]

use clap::Args;
use clap::Parser;
use clap::Subcommand;
use clap::error::ErrorKind;

// ---- command-line surface --------------------------------------------------

fn parse_define(raw: &str) -> Result<String, String> {
    if raw.contains('=') {
        Ok(raw.to_owned())
    } else {
        Err(format!("invalid define `{raw}`: expected key=value"))
    }
}

#[derive(Parser, Debug, PartialEq, Clone)]
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
    command: RootCommands,
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
enum RootCommands {
    #[command(about = "Serve traffic")]
    Serve(ServeArgs),
    #[command(about = "Check a project")]
    Check(CheckArgs),
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args, Debug, PartialEq, Clone)]
#[command(override_usage = "tool serve [OPTIONS] <COMMAND>")]
struct ServeArgs {
    #[command(subcommand)]
    command: ServeCommands,
}

#[derive(Subcommand, Debug, PartialEq, Clone)]
enum ServeCommands {
    #[command(about = "Run a worker")]
    Worker(WorkerArgs),
    #[command(external_subcommand)]
    External(Vec<String>),
}

#[derive(Args, Debug, PartialEq, Clone)]
#[command(override_usage = "tool serve worker [OPTIONS] --port <PORT> [-- <TRAILING>...]")]
struct WorkerArgs {
    #[arg(
        long,
        value_name = "PORT",
        required = true,
        value_parser = clap::value_parser!(u16).range(1..)
    )]
    port: u16,
    #[arg(long, conflicts_with = "udp")]
    tcp: bool,
    #[arg(long, conflicts_with = "tcp")]
    udp: bool,
    #[arg(last = true)]
    trailing: Vec<String>,
}

#[derive(Args, Debug, PartialEq, Clone)]
#[command(override_usage = "tool check [OPTIONS] [PATH]")]
struct CheckArgs {
    #[arg(long)]
    strict: bool,
    #[arg(value_name = "PATH")]
    path: Option<String>,
}

// ---- observed outcome -------------------------------------------------------

/// Every observation the tests make, flattened out of the derived tree so the
/// assertions read against the spec rather than the enum nesting.
#[derive(Debug, PartialEq, Clone)]
struct State {
    config: String,
    define: Vec<String>,
    outcome: Outcome,
}

#[derive(Debug, PartialEq, Clone)]
enum Outcome {
    /// Root-level plugin: the external name plus its verbatim tokens.
    RootPlugin {
        name: String,
        tokens: Vec<String>,
    },
    Serve(ServeOutcome),
    Check {
        strict: bool,
        path: Option<String>,
    },
}

#[derive(Debug, PartialEq, Clone)]
enum ServeOutcome {
    /// Serve-level plugin.
    Plugin { name: String, tokens: Vec<String> },
    Worker {
        port: u16,
        tcp: bool,
        udp: bool,
        trailing: Vec<String>,
    },
}

impl State {
    fn from_cli(cli: Cli) -> Self {
        let Cli {
            config,
            define,
            command,
        } = cli;
        let outcome = match command {
            RootCommands::External(tokens) => {
                let (name, tokens) = split_name(tokens);
                Outcome::RootPlugin { name, tokens }
            }
            RootCommands::Check(CheckArgs { strict, path }) => Outcome::Check { strict, path },
            RootCommands::Serve(ServeArgs { command }) => Outcome::Serve(match command {
                ServeCommands::External(tokens) => {
                    let (name, tokens) = split_name(tokens);
                    ServeOutcome::Plugin { name, tokens }
                }
                ServeCommands::Worker(WorkerArgs {
                    port,
                    tcp,
                    udp,
                    trailing,
                }) => ServeOutcome::Worker {
                    port,
                    tcp,
                    udp,
                    trailing,
                },
            }),
        };
        Self {
            config,
            define,
            outcome,
        }
    }
}

fn split_name(tokens: Vec<String>) -> (String, Vec<String>) {
    assert!(
        !tokens.is_empty(),
        "external capture must always include the command name"
    );
    let mut iter = tokens.into_iter();
    let name = iter.next().unwrap();
    (name, iter.collect())
}

// ---- parse helpers ----------------------------------------------------------

fn parse(argv: &[&str]) -> Result<State, clap::Error> {
    Cli::try_parse_from(argv).map(State::from_cli)
}

fn ok(argv: &[&str]) -> State {
    parse(argv).unwrap_or_else(|err| panic!("expected success for {argv:?}: {err}"))
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

struct Failure {
    kind: ErrorKind,
    text: String,
}

fn fail(argv: &[&str]) -> Failure {
    match parse(argv) {
        Ok(state) => panic!("expected failure for {argv:?}, got {state:?}"),
        Err(err) => Failure {
            kind: err.kind(),
            text: rendered(&err),
        },
    }
}

fn help(argv: &[&str]) -> (ErrorKind, String) {
    match parse(argv) {
        Ok(state) => panic!("expected help display for {argv:?}, got {state:?}"),
        Err(err) => {
            assert_eq!(err.kind(), ErrorKind::DisplayHelp, "{argv:?}: {err}");
            (err.kind(), rendered(&err))
        }
    }
}

// ===========================================================================
// Root external capture
// ===========================================================================

#[test]
fn root_external_uses_defaults_and_captures_name_and_args() {
    let state = ok(&["tool", "plug", "arg"]);
    assert_eq!(state.config, "builtin.toml");
    assert_eq!(state.define, Vec::<String>::new());
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "plug".to_owned(),
            tokens: vec!["arg".to_owned()],
        }
    );
}

#[test]
fn root_explicit_config_and_ordered_defines() {
    let state = ok(&[
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
    assert_eq!(state.config, "custom.toml");
    assert_eq!(state.define, vec!["a=1", "b=2", "c=3"]);
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "plug".to_owned(),
            tokens: vec![],
        }
    );
}

#[test]
fn root_external_captures_every_token_after_name_verbatim() {
    let state = ok(&[
        "tool",
        "--config",
        "before.toml",
        "plug",
        "--flag",
        "serve",
        "check",
        "worker",
        "--config",
        "after.toml",
        "--define",
        "k=v",
        "--",
        "",
    ]);
    assert_eq!(state.config, "before.toml");
    assert_eq!(state.define, Vec::<String>::new());
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "plug".to_owned(),
            tokens: vec![
                "--flag".to_owned(),
                "serve".to_owned(),
                "check".to_owned(),
                "worker".to_owned(),
                "--config".to_owned(),
                "after.toml".to_owned(),
                "--define".to_owned(),
                "k=v".to_owned(),
                "--".to_owned(),
                String::new(),
            ],
        }
    );
}

#[test]
fn root_unknown_name_is_external_even_when_serve_knows_it() {
    // `check` is declared at root; a prefix of it and a name only serve knows
    // (`worker`) both belong to the root catch-all.
    for argv in [
        &["tool", "chek"][..],
        &["tool", "worker", "--port", "80"][..],
        &["tool", "serv", "x"][..],
    ] {
        let state = ok(argv);
        let Outcome::RootPlugin { name, .. } = &state.outcome else {
            panic!("{argv:?} expected root plugin, got {:?}", state.outcome);
        };
        assert_eq!(name, argv[1], "{argv:?}");
    }
}

#[test]
fn root_double_dash_names_external_command() {
    let state = ok(&["tool", "--config", "c.toml", "--", "plug", "x", "--", ""]);
    assert_eq!(state.config, "c.toml");
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "plug".to_owned(),
            tokens: vec!["x".to_owned(), "--".to_owned(), String::new()],
        }
    );

    // The first post-separator token is the name even if it looks like an
    // option, is a declared command, is another separator, or is empty.
    let state = ok(&["tool", "--", "--flag", "x"]);
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "--flag".to_owned(),
            tokens: vec!["x".to_owned()],
        }
    );
    let state = ok(&["tool", "--", "serve", "--port"]);
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "serve".to_owned(),
            tokens: vec!["--port".to_owned()],
        }
    );
    let state = ok(&["tool", "--", "--", "--"]);
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "--".to_owned(),
            tokens: vec!["--".to_owned()],
        }
    );
    let state = ok(&["tool", "--", "", "rest"]);
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: String::new(),
            tokens: vec!["rest".to_owned()],
        }
    );
}

// ===========================================================================
// serve: declared worker vs serve plugin capture
// ===========================================================================

#[test]
fn serve_plugin_captures_unknown_name_and_following_tokens() {
    let state = ok(&["tool", "serve", "plug", "--flag", "worker", "check"]);
    assert_eq!(state.config, "builtin.toml");
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "plug".to_owned(),
            tokens: vec!["--flag".to_owned(), "worker".to_owned(), "check".to_owned(),],
        })
    );
}

#[test]
fn serve_declared_worker_wins_over_serve_catch_all() {
    let state = ok(&["tool", "serve", "worker", "--port", "8080"]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Worker {
            port: 8080,
            tcp: false,
            udp: false,
            trailing: vec![],
        })
    );

    // A mere prefix of `worker` is a serve plugin; the declared name is exact.
    let state = ok(&["tool", "serve", "work", "--port", "80"]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "work".to_owned(),
            tokens: vec!["--port".to_owned(), "80".to_owned()],
        })
    );

    // `check` is not declared under serve, so serve's own catch-all takes it
    // rather than the root command being invoked.
    let state = ok(&["tool", "serve", "check"]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "check".to_owned(),
            tokens: vec![],
        })
    );
}

#[test]
fn serve_double_dash_names_a_serve_plugin_not_trailing_worker_data() {
    // `serve` has no last-positional: its separator forces the next token to
    // be its external name, including declared-looking or empty names.
    let state = ok(&["tool", "serve", "--", "worker", "--port"]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "worker".to_owned(),
            tokens: vec!["--port".to_owned()],
        })
    );
    let state = ok(&["tool", "serve", "--", "", "x"]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: String::new(),
            tokens: vec!["x".to_owned()],
        })
    );
}

#[test]
fn serve_plugin_tokens_cannot_be_stolen_by_globals() {
    let state = ok(&[
        "tool",
        "--config",
        "before.toml",
        "serve",
        "plug",
        "--config",
        "after.toml",
        "--define",
        "k=v",
    ]);
    assert_eq!(state.config, "before.toml");
    assert_eq!(state.define, Vec::<String>::new());
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "plug".to_owned(),
            tokens: vec![
                "--config".to_owned(),
                "after.toml".to_owned(),
                "--define".to_owned(),
                "k=v".to_owned(),
            ],
        })
    );
}

// ===========================================================================
// worker: port, protocol switches, trailing arguments
// ===========================================================================

#[test]
fn worker_accepts_port_protocol_and_trailing() {
    let state = ok(&[
        "tool", "serve", "worker", "--port", "8080", "--tcp", "--", "curl", "--flag",
    ]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Worker {
            port: 8080,
            tcp: true,
            udp: false,
            trailing: vec!["curl".to_owned(), "--flag".to_owned()],
        })
    );

    let state = ok(&["tool", "serve", "worker", "--udp", "--port=1"]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Worker {
            port: 1,
            tcp: false,
            udp: true,
            trailing: vec![],
        })
    );

    let state = ok(&["tool", "serve", "worker", "--port", "65535"]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Worker {
            port: 65535,
            tcp: false,
            udp: false,
            trailing: vec![],
        })
    );
}

#[test]
fn worker_trailing_after_its_own_separator_is_verbatim_data() {
    let state = ok(&[
        "tool", "serve", "worker", "--port", "80", "--", "plug", "--tcp", "--udp", "serve", "--",
        "",
    ]);
    let Outcome::Serve(ServeOutcome::Worker {
        trailing, tcp, udp, ..
    }) = &state.outcome
    else {
        panic!("expected worker, got {:?}", state.outcome);
    };
    assert!(!tcp && !udp, "trailing switches must not parse as flags");
    assert_eq!(trailing, &["plug", "--tcp", "--udp", "serve", "--", ""]);
}

#[test]
fn worker_separator_cannot_redispatch_a_plugin() {
    let state = ok(&[
        "tool", "serve", "worker", "--port", "80", "--", "plug", "--config", "x.toml",
    ]);
    assert_eq!(state.config, "builtin.toml");
    assert_eq!(state.define, Vec::<String>::new());
    assert!(matches!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Worker { .. })
    ));
}

// ===========================================================================
// check
// ===========================================================================

#[test]
fn check_accepts_only_optional_path_and_strict() {
    let state = ok(&["tool", "check"]);
    assert_eq!(
        state.outcome,
        Outcome::Check {
            strict: false,
            path: None,
        }
    );

    let state = ok(&["tool", "check", "--strict", "path/to/file"]);
    assert_eq!(
        state.outcome,
        Outcome::Check {
            strict: true,
            path: Some("path/to/file".to_owned()),
        }
    );

    // Options may follow or precede the single positional.
    let state = ok(&["tool", "check", "p", "--strict"]);
    assert_eq!(
        state.outcome,
        Outcome::Check {
            strict: true,
            path: Some("p".to_owned()),
        }
    );
}

// ===========================================================================
// Globals at every level
// ===========================================================================

#[test]
fn globals_can_appear_before_between_or_after_command_tokens() {
    // The same globals distributed across root/serve/worker parse identically:
    // explicit config wins over the default and append-style definitions keep
    // overall command-line order regardless of the level at which they appear.
    let orderings: [&[&str]; 4] = [
        &[
            "tool", "--config", "c.toml", "--define", "a=1", "serve", "--define", "b=2", "worker",
            "--port", "8080", "--define", "c=3",
        ],
        &[
            "tool", "serve", "--config", "c.toml", "--define", "a=1", "worker", "--port", "8080",
            "--define", "b=2", "--define", "c=3",
        ],
        &[
            "tool", "serve", "worker", "--config", "c.toml", "--port", "8080", "--define", "a=1",
            "--define", "b=2", "--define", "c=3",
        ],
        &[
            "tool", "--define", "a=1", "serve", "worker", "--port", "8080", "--config", "c.toml",
            "--define", "b=2", "--define", "c=3",
        ],
    ];

    let expected = State {
        config: "c.toml".to_owned(),
        define: vec!["a=1".to_owned(), "b=2".to_owned(), "c=3".to_owned()],
        outcome: Outcome::Serve(ServeOutcome::Worker {
            port: 8080,
            tcp: false,
            udp: false,
            trailing: vec![],
        }),
    };
    for argv in orderings {
        assert_eq!(ok(argv), expected, "{argv:?}");
    }
}

#[test]
fn globals_reach_check_and_both_plugin_levels() {
    let state = ok(&[
        "tool", "--define", "a=1", "check", "--strict", "--config", "x.toml",
    ]);
    assert_eq!(state.config, "x.toml");
    assert_eq!(state.define, vec!["a=1"]);
    assert_eq!(
        state.outcome,
        Outcome::Check {
            strict: true,
            path: None,
        }
    );

    let state = ok(&["tool", "--define", "a=1", "plug"]);
    assert_eq!(state.define, vec!["a=1"]);
    assert!(matches!(state.outcome, Outcome::RootPlugin { .. }));

    let state = ok(&["tool", "serve", "--define", "a=1", "plug"]);
    assert_eq!(state.define, vec!["a=1"]);
    assert!(matches!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin { .. })
    ));
}

// ===========================================================================
// Failures
// ===========================================================================

#[test]
fn worker_missing_port_fails_in_worker_context() {
    for argv in [
        &["tool", "serve", "worker"][..],
        &["tool", "serve", "worker", "--tcp"][..],
    ] {
        let f = fail(argv);
        assert_eq!(f.kind, ErrorKind::MissingRequiredArgument, "{argv:?}");
        assert!(f.text.contains("--port"), "{argv:?}: {}", f.text);
        assert!(f.text.contains("tool serve worker"), "{argv:?}: {}", f.text);
        assert!(
            !f.text.contains("tool serve [OPTIONS] <COMMAND>"),
            "{argv:?} used serve usage:\n{}",
            f.text
        );
    }
}

#[test]
fn worker_port_out_of_range_and_non_numeric_fail() {
    for argv in [
        &["tool", "serve", "worker", "--port", "0"][..],
        &["tool", "serve", "worker", "--port=0"][..],
        &["tool", "serve", "worker", "--port", "65536"][..],
        &["tool", "serve", "worker", "--port=99999"][..],
        &["tool", "serve", "worker", "--port=-1"][..],
        &["tool", "serve", "worker", "--port", "abc"][..],
        &["tool", "serve", "worker", "--port=80x"][..],
    ] {
        let f = fail(argv);
        assert_eq!(f.kind, ErrorKind::ValueValidation, "{argv:?}");
        // The error names the worker-specific argument, which pins the most
        // specific command context, and quotes the triggering value.
        assert!(f.text.contains("--port <PORT>"), "{argv:?}: {}", f.text);
        let bad = argv
            .iter()
            .rev()
            .find(|t| **t != "--port")
            .map(|t| t.trim_start_matches("--port="))
            .unwrap();
        assert!(
            f.text.contains(&format!("'{bad}'")),
            "{argv:?} did not quote `{bad}`:\n{}",
            f.text
        );
    }

    // A negative value passed as a separate token looks like an option and is
    // reported as unknown rather than silently parsed.
    let f = fail(&["tool", "serve", "worker", "--port", "-1"]);
    assert_eq!(f.kind, ErrorKind::UnknownArgument);
    assert!(f.text.contains("-1"), "{}", f.text);
    assert!(f.text.contains("tool serve worker"), "{}", f.text);
}

#[test]
fn worker_protocol_conflict_fails() {
    for argv in [
        &["tool", "serve", "worker", "--port", "80", "--tcp", "--udp"][..],
        &["tool", "serve", "worker", "--udp", "--tcp", "--port", "80"][..],
    ] {
        let f = fail(argv);
        assert_eq!(f.kind, ErrorKind::ArgumentConflict, "{argv:?}");
        assert!(f.text.contains("--tcp"), "{argv:?}: {}", f.text);
        assert!(f.text.contains("--udp"), "{argv:?}: {}", f.text);
        assert!(f.text.contains("tool serve worker"), "{argv:?}: {}", f.text);
    }
}

#[test]
fn duplicate_non_repeatable_arguments_fail_at_their_level() {
    let f = fail(&["tool", "serve", "worker", "--port", "80", "--port", "81"]);
    assert_eq!(f.kind, ErrorKind::ArgumentConflict);
    assert!(f.text.contains("--port"), "{}", f.text);
    assert!(f.text.contains("tool serve worker"), "{}", f.text);

    // Global config is non-repeatable; the most specific command context wins.
    let f = fail(&["tool", "--config", "a", "--config", "b", "plug"]);
    assert_eq!(f.kind, ErrorKind::ArgumentConflict);
    assert!(f.text.contains("--config"), "{}", f.text);
    assert!(f.text.contains("tool [OPTIONS] <COMMAND>"), "{}", f.text);

    let f = fail(&[
        "tool", "serve", "worker", "--config", "a", "--config", "b", "--port", "80",
    ]);
    assert_eq!(f.kind, ErrorKind::ArgumentConflict);
    assert!(f.text.contains("--config"), "{}", f.text);
    assert!(f.text.contains("tool serve worker"), "{}", f.text);
}

#[test]
fn invalid_define_fails_before_external_name_but_is_data_after_it() {
    for argv in [
        &["tool", "--define", "nokey", "plug"][..],
        &["tool", "--define=nokey"][..],
        &["tool", "serve", "--define", "nokey", "plug"][..],
        &[
            "tool", "serve", "worker", "--port", "80", "--define", "nokey",
        ][..],
    ] {
        let f = fail(argv);
        assert_eq!(f.kind, ErrorKind::ValueValidation, "{argv:?}");
        assert!(f.text.contains("nokey"), "{argv:?}: {}", f.text);
        assert!(f.text.contains("--define"), "{argv:?}: {}", f.text);
    }

    // Once the external name is selected, an invalid define is plugin data.
    let state = ok(&["tool", "plug", "--define", "nokey"]);
    assert_eq!(state.define, Vec::<String>::new());
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "plug".to_owned(),
            tokens: vec!["--define".to_owned(), "nokey".to_owned()],
        }
    );
    let state = ok(&["tool", "serve", "plug", "--define", "nokey"]);
    assert_eq!(state.define, Vec::<String>::new());
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "plug".to_owned(),
            tokens: vec!["--define".to_owned(), "nokey".to_owned()],
        })
    );
}

#[test]
fn missing_config_value_fails_on_config() {
    for argv in [
        &["tool", "--config"][..],
        &["tool", "serve", "--config"][..],
        &["tool", "serve", "worker", "--port", "80", "--config"][..],
        &["tool", "check", "--config"][..],
        &["tool", "--define"][..],
    ] {
        let f = fail(argv);
        assert_eq!(f.kind, ErrorKind::InvalidValue, "{argv:?}");
        let needle = if argv.last() == Some(&"--define") {
            "--define"
        } else {
            "--config"
        };
        assert!(f.text.contains(needle), "{argv:?}: {}", f.text);
    }
}

#[test]
fn unknown_options_fail_in_the_most_specific_context() {
    let f = fail(&["tool", "--bogus", "plug"]);
    assert_eq!(f.kind, ErrorKind::UnknownArgument);
    assert!(f.text.contains("--bogus"), "{}", f.text);
    assert!(f.text.contains("tool [OPTIONS] <COMMAND>"), "{}", f.text);

    let f = fail(&["tool", "serve", "--bogus", "plug"]);
    assert_eq!(f.kind, ErrorKind::UnknownArgument);
    assert!(f.text.contains("--bogus"), "{}", f.text);
    assert!(
        f.text.contains("tool serve [OPTIONS] <COMMAND>"),
        "{}",
        f.text
    );

    let f = fail(&["tool", "serve", "worker", "--bogus", "--port", "80"]);
    assert_eq!(f.kind, ErrorKind::UnknownArgument);
    assert!(f.text.contains("--bogus"), "{}", f.text);
    assert!(f.text.contains("tool serve worker"), "{}", f.text);

    let f = fail(&["tool", "-x", "plug"]);
    assert_eq!(f.kind, ErrorKind::UnknownArgument);
    assert!(f.text.contains("-x"), "{}", f.text);
}

#[test]
fn worker_and_serve_arguments_are_unknown_to_check() {
    for argv in [
        &["tool", "check", "--port", "80"][..],
        &["tool", "check", "--tcp"][..],
        &["tool", "check", "--udp"][..],
    ] {
        let f = fail(argv);
        assert_eq!(f.kind, ErrorKind::UnknownArgument, "{argv:?}");
        assert!(
            f.text.contains("tool check"),
            "{argv:?} lost check context:\n{}",
            f.text
        );
    }

    // check takes at most one path.
    let f = fail(&["tool", "check", "a", "b"]);
    assert_eq!(f.kind, ErrorKind::UnknownArgument);
    assert!(f.text.contains("'b'"), "{}", f.text);
    assert!(f.text.contains("tool check"), "{}", f.text);
}

#[test]
fn worker_does_not_accept_bare_positional_before_separator() {
    let f = fail(&["tool", "serve", "worker", "--port", "80", "tail"]);
    assert_eq!(f.kind, ErrorKind::UnknownArgument);
    assert!(f.text.contains("'tail'"), "{}", f.text);
    assert!(f.text.contains("tool serve worker"), "{}", f.text);
}

// ===========================================================================
// Help at every level
// ===========================================================================

#[test]
fn root_help_succeeds_without_subcommand_or_required_values() {
    let (_, text) = help(&["tool", "--help"]);
    for needle in [
        "tool [OPTIONS] <COMMAND>",
        "serve",
        "check",
        "--config",
        "builtin.toml",
        "--define",
        "key=value",
    ] {
        assert!(
            text.contains(needle),
            "root help missing `{needle}`:\n{text}"
        );
    }
    // External variants are a catch-all boundary, not listed command names...
    assert!(
        !text.contains("--port") && !text.contains("--strict"),
        "subcommand options leaked into root help:\n{text}"
    );
    assert!(!text.to_ascii_lowercase().contains("error"), "{text}");
}

#[test]
fn serve_help_shows_worker_and_catch_all_boundary_only() {
    let (_, text) = help(&["tool", "serve", "--help"]);
    assert!(text.contains("tool serve [OPTIONS] <COMMAND>"), "{text}");
    assert!(text.contains("worker"), "{text}");
    assert!(text.contains("builtin.toml"), "{text}");
    assert!(
        !text.contains("check"),
        "root `check` leaked into serve help:\n{text}"
    );
    assert!(
        !text.contains("--port") && !text.contains("--tcp"),
        "worker options leaked into serve help:\n{text}"
    );
    assert!(!text.to_ascii_lowercase().contains("error"), "{text}");
}

#[test]
fn worker_help_succeeds_while_port_is_missing() {
    let (_, text) = help(&["tool", "serve", "worker", "--help"]);
    assert!(text.contains("tool serve worker"), "{text}");
    for needle in ["--port", "PORT", "--tcp", "--udp", "TRAILING"] {
        assert!(
            text.contains(needle),
            "worker help missing `{needle}`:\n{text}"
        );
    }
    assert!(
        !text.contains("--strict"),
        "check option leaked into worker help:\n{text}"
    );
    assert!(!text.to_ascii_lowercase().contains("error"), "{text}");
}

#[test]
fn check_help_succeeds_and_shows_only_check_surface() {
    let (_, text) = help(&["tool", "check", "--help"]);
    assert!(text.contains("tool check"), "{text}");
    assert!(text.contains("--strict"), "{text}");
    assert!(text.contains("PATH"), "{text}");
    assert!(
        !text.contains("--port") && !text.contains("--tcp") && !text.contains("--udp"),
        "worker options leaked into check help:\n{text}"
    );
    assert!(!text.to_ascii_lowercase().contains("error"), "{text}");
}

// ===========================================================================
// Isolation and global-argument placement
// ===========================================================================

#[test]
fn fixed_sequence_is_mutually_isolated() {
    let clean_root_help = help(&["tool", "--help"]).1;
    let clean_serve_help = help(&["tool", "serve", "--help"]).1;
    let clean_worker_help = help(&["tool", "serve", "worker", "--help"]).1;
    let clean_check_help = help(&["tool", "check", "--help"]).1;

    // 1. Root plugin success with globals before the name.
    let state = ok(&[
        "tool",
        "--config",
        "custom.toml",
        "--define",
        "a=1",
        "plug",
        "--flag",
        "serve",
        "--",
        "",
    ]);
    assert_eq!(state.config, "custom.toml");
    assert_eq!(state.define, vec!["a=1"]);
    assert_eq!(
        state.outcome,
        Outcome::RootPlugin {
            name: "plug".to_owned(),
            tokens: vec![
                "--flag".to_owned(),
                "serve".to_owned(),
                "--".to_owned(),
                String::new(),
            ],
        }
    );

    // 2. Serve plugin success.
    let state = ok(&[
        "tool", "serve", "--define", "b=2", "plug", "worker", "--tcp",
    ]);
    assert_eq!(state.config, "builtin.toml");
    assert_eq!(state.define, vec!["b=2"]);
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "plug".to_owned(),
            tokens: vec!["worker".to_owned(), "--tcp".to_owned()],
        })
    );

    // 3. Invalid input fails categorically.
    let f = fail(&["tool", "serve", "worker", "--port", "nope"]);
    assert_eq!(f.kind, ErrorKind::ValueValidation);
    assert!(f.text.contains("nope"), "{}", f.text);

    // 4. Worker success afterwards sees fresh defaults and no leaked tokens.
    let state = ok(&["tool", "serve", "worker", "--port", "9", "--", "z", "--tcp"]);
    assert_eq!(state.config, "builtin.toml");
    assert_eq!(state.define, Vec::<String>::new());
    assert_eq!(
        state.outcome,
        Outcome::Serve(ServeOutcome::Worker {
            port: 9,
            tcp: false,
            udp: false,
            trailing: vec!["z".to_owned(), "--tcp".to_owned()],
        })
    );

    // 5. All four help texts are byte-for-byte their clean versions.
    for (argv, clean) in [
        (&["tool", "--help"][..], &clean_root_help),
        (&["tool", "serve", "--help"][..], &clean_serve_help),
        (
            &["tool", "serve", "worker", "--help"][..],
            &clean_worker_help,
        ),
        (&["tool", "check", "--help"][..], &clean_check_help),
    ] {
        let text = help(argv).1;
        assert_eq!(&text, clean, "{argv:?} help was polluted:\n{text}");
        assert!(!text.contains("nope"), "{argv:?}:\n{text}");
    }
}

#[test]
fn moving_globals_does_not_change_structure_order_or_usage() {
    let variants: [&[&str]; 3] = [
        &[
            "tool", "--config", "x.toml", "--define", "a=1", "serve", "--define", "b=2", "plug",
            "x",
        ],
        &[
            "tool", "serve", "--config", "x.toml", "--define", "a=1", "--define", "b=2", "plug",
            "x",
        ],
        &[
            "tool", "serve", "plug", "x", "--config", "x.toml", "--define", "a=1",
        ],
    ];

    // Moving globals only while they stay before the plugin name yields the
    // exact same structure.
    let pre_name = ok(variants[0]);
    assert_eq!(ok(variants[1]), pre_name);
    assert_eq!(pre_name.config, "x.toml");
    assert_eq!(pre_name.define, vec!["a=1", "b=2"]);
    assert_eq!(
        pre_name.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "plug".to_owned(),
            tokens: vec!["x".to_owned()],
        })
    );

    // A global placed *after* the plugin name is captured verbatim instead, so
    // moving it across the boundary visibly (and correctly) changes parsing.
    let after_name = ok(variants[2]);
    assert_eq!(after_name.config, "builtin.toml");
    assert_eq!(after_name.define, Vec::<String>::new());
    assert_eq!(
        after_name.outcome,
        Outcome::Serve(ServeOutcome::Plugin {
            name: "plug".to_owned(),
            tokens: vec![
                "x".to_owned(),
                "--config".to_owned(),
                "x.toml".to_owned(),
                "--define".to_owned(),
                "a=1".to_owned(),
            ],
        })
    );

    // Error usage is stable regardless of global placement.
    for argv in [
        &["tool", "--config", "c.toml", "serve", "worker"][..],
        &["tool", "serve", "--config", "c.toml", "worker"][..],
        &["tool", "serve", "worker", "--config", "c.toml"][..],
    ] {
        let f = fail(argv);
        assert_eq!(f.kind, ErrorKind::MissingRequiredArgument, "{argv:?}");
        assert!(
            f.text.contains("tool serve worker [OPTIONS] --port <PORT>"),
            "{argv:?}:\n{}",
            f.text
        );
    }
}
