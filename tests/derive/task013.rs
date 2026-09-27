// Derived-CLI regression tests for nested subcommands interacting with global
// arguments, `--` trailing sequences, and failure recovery.
//
// Everything is observed through `Cli::try_parse_from` with argv0 included.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap::error::ErrorKind;

#[derive(Parser, PartialEq, Debug)]
#[command(name = "tool", bin_name = "tool")]
struct Cli {
    #[arg(long, global = true, default_value = "builtin.toml", value_name = "path")]
    config: PathBuf,
    #[arg(long = "define", global = true, value_name = "key=value")]
    define: Vec<String>,
    #[command(subcommand)]
    command: TopCommand,
}

#[derive(Subcommand, PartialEq, Debug)]
enum TopCommand {
    Serve(ServeArgs),
    Check(CheckArgs),
}

#[derive(Args, PartialEq, Debug)]
struct ServeArgs {
    #[arg(long)]
    profile: Option<String>,
    #[command(subcommand)]
    command: Option<ServeCommand>,
}

#[derive(Subcommand, PartialEq, Debug)]
enum ServeCommand {
    Worker(WorkerArgs),
}

#[derive(Args, PartialEq, Debug)]
struct WorkerArgs {
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
    #[arg(long, conflicts_with = "https")]
    http: bool,
    #[arg(long)]
    https: bool,
    #[arg(last = true)]
    trailing: Vec<String>,
}

#[derive(Args, PartialEq, Debug)]
struct CheckArgs {
    path: Option<PathBuf>,
    #[arg(long)]
    strict: bool,
}

fn serve(cli: &Cli) -> &ServeArgs {
    match &cli.command {
        TopCommand::Serve(args) => args,
        other => panic!("expected serve, got {other:?}"),
    }
}

fn worker(cli: &Cli) -> &WorkerArgs {
    match &serve(cli).command {
        Some(ServeCommand::Worker(args)) => args,
        other => panic!("expected serve worker, got {other:?}"),
    }
}

fn check(cli: &Cli) -> &CheckArgs {
    match &cli.command {
        TopCommand::Check(args) => args,
        other => panic!("expected check, got {other:?}"),
    }
}

#[track_caller]
fn ok(argv: &[&str]) -> Cli {
    match Cli::try_parse_from(argv) {
        Ok(cli) => cli,
        Err(err) => panic!("expected success for {argv:?}, got:\n{err}"),
    }
}

#[track_caller]
fn fail(argv: &[&str]) -> clap::Error {
    match Cli::try_parse_from(argv) {
        Ok(cli) => panic!("expected failure for {argv:?}, got {cli:?}"),
        Err(err) => err,
    }
}

#[test]
fn defaults_apply_without_globals() {
    let cli = ok(&["tool", "serve"]);
    assert_eq!(cli.config, PathBuf::from("builtin.toml"));
    assert_eq!(cli.define, Vec::<String>::new());
    assert_eq!(serve(&cli).profile, None);
    assert_eq!(serve(&cli).command, None);
}

#[test]
fn explicit_config_overrides_default() {
    let cli = ok(&["tool", "--config", "custom.toml", "serve"]);
    assert_eq!(cli.config, PathBuf::from("custom.toml"));
}

#[test]
fn global_args_accepted_at_every_level() {
    let expected = ok(&[
        "tool", "--config", "a.toml", "--define", "x=1", "serve", "--profile", "dev", "worker",
        "--port", "8080",
    ]);

    // Same tokens, globals relocated to each level.
    for argv in [
        &["tool", "serve", "--config", "a.toml", "--define", "x=1", "--profile", "dev", "worker", "--port", "8080"][..],
        &["tool", "serve", "--profile", "dev", "worker", "--config", "a.toml", "--define", "x=1", "--port", "8080"][..],
        &["tool", "serve", "--profile", "dev", "worker", "--port", "8080", "--config", "a.toml", "--define", "x=1"][..],
        &["tool", "--define", "x=1", "serve", "--config", "a.toml", "--profile", "dev", "worker", "--port", "8080"][..],
    ] {
        assert_eq!(ok(argv), expected, "argv: {argv:?}");
    }

    // Globals around `check`.
    let check_expected = ok(&["tool", "--config", "a.toml", "check", "--strict"]);
    for argv in [
        &["tool", "check", "--config", "a.toml", "--strict"][..],
        &["tool", "check", "--strict", "--config", "a.toml"][..],
    ] {
        assert_eq!(ok(argv), check_expected, "argv: {argv:?}");
    }
}

#[test]
fn defines_preserve_order_across_levels() {
    let cli = ok(&[
        "tool", "--define", "a=1", "serve", "--define", "b=2", "worker", "--port", "1",
        "--define", "c=3",
    ]);
    assert_eq!(cli.define, vec!["a=1", "b=2", "c=3"]);
}

#[test]
fn worker_accepts_full_shape() {
    let cli = ok(&[
        "tool", "serve", "--profile", "prod", "worker", "--port", "443", "--https",
    ]);
    let worker = worker(&cli);
    assert_eq!(serve(&cli).profile.as_deref(), Some("prod"));
    assert_eq!(worker.port, 443);
    assert!(!worker.http);
    assert!(worker.https);
    assert!(worker.trailing.is_empty());
}

#[test]
fn worker_port_bounds() {
    assert_eq!(worker(&ok(&["tool", "serve", "worker", "--port", "1"])).port, 1);
    assert_eq!(
        worker(&ok(&["tool", "serve", "worker", "--port", "65535"])).port,
        65535
    );

    for argv in [
        &["tool", "serve", "worker", "--port", "0"][..],
        &["tool", "serve", "worker", "--port", "65536"][..],
        &["tool", "serve", "worker", "--port", "abc"][..],
        &["tool", "serve", "worker", "--port", "12x"][..],
        &["tool", "serve", "worker", "--port", "1.5"][..],
        &["tool", "serve", "worker", "--port=-1"][..],
    ] {
        let err = fail(argv);
        assert_eq!(err.kind(), ErrorKind::ValueValidation, "argv: {argv:?}");
        let msg = err.to_string();
        assert!(msg.contains("--port"), "missing trigger token: {msg}");
    }
}

#[test]
fn worker_trailing_sequence_after_double_dash() {
    let cli = ok(&[
        "tool", "serve", "worker", "--port", "9", "--", "--http", "serve", "check",
        "--define", "k=v", "--", "--port",
    ]);
    assert_eq!(
        worker(&cli).trailing,
        vec!["--http", "serve", "check", "--define", "k=v", "--", "--port"]
    );
}

#[test]
fn worker_rejects_unknown_token_before_separator() {
    let err = fail(&["tool", "serve", "worker", "--port", "9", "bogus"]);
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    let msg = err.to_string();
    assert!(msg.contains("bogus"), "missing trigger token: {msg}");
    assert!(msg.contains("tool serve worker"), "missing usage context: {msg}");

    // Known-at-other-levels tokens are still unknown here before `--`.
    fail(&["tool", "serve", "worker", "--port", "9", "--profile", "x"]);
    fail(&["tool", "serve", "worker", "--port", "9", "--strict"]);
}

#[test]
fn worker_required_unique_port() {
    let err = fail(&["tool", "serve", "worker"]);
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    let msg = err.to_string();
    assert!(msg.contains("--port"), "missing trigger token: {msg}");
    assert!(msg.contains("tool serve worker"), "missing usage context: {msg}");

    let err = fail(&["tool", "serve", "worker", "--port", "1", "--port", "2"]);
    let msg = err.to_string();
    assert!(msg.contains("--port"), "missing trigger token: {msg}");
}

#[test]
fn worker_protocol_flags_conflict_and_are_unique() {
    let err = fail(&["tool", "serve", "worker", "--port", "1", "--http", "--https"]);
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    let msg = err.to_string();
    assert!(msg.contains("--http"), "missing trigger token: {msg}");
    assert!(msg.contains("--https"), "missing conflicting token: {msg}");

    fail(&["tool", "serve", "worker", "--port", "1", "--http", "--http"]);
    fail(&["tool", "serve", "worker", "--port", "1", "--https", "--https"]);
}

#[test]
fn serve_profile_is_unique() {
    let err = fail(&["tool", "serve", "--profile", "a", "--profile", "b"]);
    let msg = err.to_string();
    assert!(msg.contains("--profile"), "missing trigger token: {msg}");
    assert!(msg.contains("tool serve"), "missing usage context: {msg}");
}

#[test]
fn check_shape_and_separator() {
    let cli = ok(&["tool", "check"]);
    assert_eq!(check(&cli).path, None);
    assert!(!check(&cli).strict);

    let cli = ok(&["tool", "check", "--strict", "src/lib.rs"]);
    assert_eq!(check(&cli).path, Some(PathBuf::from("src/lib.rs")));
    assert!(check(&cli).strict);

    // After `--`, the next token is the optional path.
    let cli = ok(&["tool", "check", "--", "--strict"]);
    assert_eq!(check(&cli).path, Some(PathBuf::from("--strict")));
    assert!(!check(&cli).strict);

    // Anything beyond the single path fails.
    let err = fail(&["tool", "check", "--", "a", "b"]);
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    assert!(err.to_string().contains("tool check"));
    fail(&["tool", "check", "a", "b"]);
}

#[test]
fn check_does_not_inherit_serve_or_worker_args() {
    for argv in [
        &["tool", "check", "--profile", "dev"][..],
        &["tool", "check", "--port", "1"][..],
        &["tool", "check", "--http"][..],
        &["tool", "check", "--https"][..],
    ] {
        let err = fail(argv);
        assert!(
            matches!(err.kind(), ErrorKind::UnknownArgument | ErrorKind::InvalidSubcommand),
            "argv: {argv:?} kind: {:?}",
            err.kind()
        );
        assert!(err.to_string().contains("tool check"), "argv: {argv:?}");
    }
}

#[test]
fn unknown_subcommand_fails() {
    let err = fail(&["tool", "frobnicate"]);
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    assert!(err.to_string().contains("frobnicate"));

    let err = fail(&["tool", "serve", "frobnicate"]);
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    assert!(err.to_string().contains("tool serve"));
}

#[test]
fn option_missing_value_fails() {
    for argv in [
        &["tool", "--config"][..],
        &["tool", "--define"][..],
        &["tool", "serve", "--profile"][..],
        &["tool", "serve", "worker", "--port"][..],
    ] {
        let err = fail(argv);
        assert_eq!(err.kind(), ErrorKind::InvalidValue, "argv: {argv:?}");
    }
}

#[test]
fn help_succeeds_at_every_level_without_required_args() {
    let cases: &[(&[&str], &[&str], &[&str])] = &[
        (
            &["tool", "--help"],
            &["Usage: tool", "--config", "builtin.toml", "--define", "serve", "check"],
            &["--port", "--profile", "--strict"],
        ),
        (
            &["tool", "serve", "--help"],
            &["Usage: tool serve", "--profile", "worker"],
            &["--port", "--strict"],
        ),
        (
            &["tool", "serve", "worker", "--help"],
            &["Usage: tool serve worker", "--port", "--http", "--https"],
            &["--profile", "--strict"],
        ),
        (
            &["tool", "check", "--help"],
            &["Usage: tool check", "--strict"],
            &["--port", "--profile", "--http", "--https"],
        ),
    ];

    for (argv, must_contain, must_not_contain) in cases {
        let err = fail(argv);
        assert_eq!(err.kind(), ErrorKind::DisplayHelp, "argv: {argv:?}");
        let msg = err.to_string();
        for needle in *must_contain {
            assert!(msg.contains(needle), "argv: {argv:?} missing {needle:?}:\n{msg}");
        }
        for needle in *must_not_contain {
            assert!(!msg.contains(needle), "argv: {argv:?} unexpected {needle:?}:\n{msg}");
        }
    }

    // `-h` short help also short-circuits before required-arg checks.
    for argv in [
        &["tool", "-h"][..],
        &["tool", "serve", "-h"][..],
        &["tool", "serve", "worker", "-h"][..],
        &["tool", "check", "-h"][..],
    ] {
        assert_eq!(fail(argv).kind(), ErrorKind::DisplayHelp, "argv: {argv:?}");
    }
}

#[test]
fn global_position_does_not_change_help_or_error_usage() {
    let baseline = fail(&["tool", "serve", "worker", "--help"]).to_string();
    let moved = fail(&["tool", "--config", "x.toml", "serve", "worker", "--help"]).to_string();
    assert_eq!(baseline, moved);

    let baseline = fail(&["tool", "serve", "worker"]).to_string();
    let moved = fail(&["tool", "--config", "x.toml", "serve", "worker"]).to_string();
    assert_eq!(baseline, moved);
}

#[test]
fn failure_leaves_no_partial_state_for_next_parse() {
    fail(&["tool", "serve", "worker", "--port", "0", "--config", "leaked.toml"]);
    fail(&["tool", "check", "--profile", "nope"]);

    let cli = ok(&["tool", "serve", "worker", "--port", "7"]);
    assert_eq!(cli.config, PathBuf::from("builtin.toml"));
    assert!(cli.define.is_empty());
    assert_eq!(worker(&cli).port, 7);
    assert!(worker(&cli).trailing.is_empty());
}
