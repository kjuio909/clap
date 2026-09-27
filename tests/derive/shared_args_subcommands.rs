// Acceptance coverage for a derive-defined command with flattened shared
// arguments, subcommands, and a trailing var-arg, mirroring builder behavior.

use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};

#[derive(Debug, PartialEq, Eq, Parser)]
#[command(name = "tool", about = "A test tool")]
struct Cli {
    #[command(flatten)]
    shared: Shared,

    #[command(subcommand)]
    command: Sub,
}

#[derive(Debug, PartialEq, Eq, Args)]
struct Shared {
    /// Path to the configuration file
    #[arg(long, global = true, default_value = "builtin.toml")]
    config: String,

    /// Define a key=value override; may be repeated
    #[arg(long = "define", value_name = "KEY=VALUE", global = true)]
    defines: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
enum Sub {
    /// Run the server
    Serve(Serve),
    /// Check a path
    Check(Check),
}

#[derive(Debug, PartialEq, Eq, Args)]
struct Serve {
    /// Port to listen on
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,

    #[command(flatten)]
    protocol: Protocol,

    /// Arguments passed through verbatim
    #[arg(trailing_var_arg = true)]
    tail: Vec<String>,
}

#[derive(Debug, PartialEq, Eq, Args)]
#[group(required = false, multiple = false)]
struct Protocol {
    /// Use plain HTTP
    #[arg(long)]
    http: bool,

    /// Use HTTPS
    #[arg(long)]
    https: bool,
}

#[derive(Debug, PartialEq, Eq, Args)]
struct Check {
    /// Path to check
    path: Option<String>,

    /// Enable strict checks
    #[arg(long)]
    strict: bool,
}

fn serve(port: u16, http: bool, https: bool, tail: &[&str]) -> Cli {
    Cli {
        shared: Shared {
            config: "builtin.toml".to_owned(),
            defines: Vec::new(),
        },
        command: Sub::Serve(Serve {
            port,
            protocol: Protocol { http, https },
            tail: tail.iter().map(|s| s.to_string()).collect(),
        }),
    }
}

#[test]
fn command_definition_is_consistent() {
    Cli::command().debug_assert();
}

#[test]
fn shared_args_parse_the_same_before_and_after_subcommand() {
    let before = Cli::try_parse_from([
        "tool", "--config", "a.toml", "--define", "x=1", "serve", "--port", "8080",
    ])
    .unwrap();
    let after = Cli::try_parse_from([
        "tool", "serve", "--port", "8080", "--config", "a.toml", "--define", "x=1",
    ])
    .unwrap();

    assert_eq!(before, after);
    assert_eq!(before.shared.config, "a.toml");
    assert_eq!(before.shared.defines, ["x=1"]);
}

#[test]
fn explicit_config_overrides_default_and_defines_keep_order() {
    let cli = Cli::try_parse_from([
        "tool", "--define", "a=1", "--define", "b=2", "check", "--config", "c.toml",
    ])
    .unwrap();

    assert_eq!(cli.shared.config, "c.toml");
    assert_eq!(cli.shared.defines, ["a=1", "b=2"]);

    // Repeated defines within a single level keep their order in both subcommands
    let cli = Cli::try_parse_from(["tool", "check", "--define", "x=1", "--define", "y=2"]).unwrap();
    assert_eq!(cli.shared.defines, ["x=1", "y=2"]);
    let cli = Cli::try_parse_from([
        "tool", "serve", "--port", "9", "--define", "x=1", "--define", "y=2",
    ])
    .unwrap();
    assert_eq!(cli.shared.defines, ["x=1", "y=2"]);

    // Different shared args may be split across the subcommand boundary
    let split = Cli::try_parse_from(["tool", "--define", "a=1", "check", "--config", "c.toml"])
        .unwrap();
    let together =
        Cli::try_parse_from(["tool", "check", "--config", "c.toml", "--define", "a=1"]).unwrap();
    assert_eq!(split, together);
}

#[test]
fn defaults_apply_when_shared_args_are_absent() {
    let cli = Cli::try_parse_from(["tool", "check"]).unwrap();

    assert_eq!(cli.shared.config, "builtin.toml");
    assert!(cli.shared.defines.is_empty());
    assert_eq!(
        cli.command,
        Sub::Check(Check {
            path: None,
            strict: false
        })
    );
}

#[test]
fn check_accepts_optional_path_and_strict() {
    let cli = Cli::try_parse_from(["tool", "check", "./src", "--strict"]).unwrap();

    assert_eq!(
        cli.command,
        Sub::Check(Check {
            path: Some("./src".to_owned()),
            strict: true,
        })
    );
}

#[test]
fn serve_parses_port_and_protocol() {
    assert_eq!(
        Cli::try_parse_from(["tool", "serve", "--port", "1"]).unwrap(),
        serve(1, false, false, &[])
    );
    assert_eq!(
        Cli::try_parse_from(["tool", "serve", "--port", "65535", "--http"]).unwrap(),
        serve(65535, true, false, &[])
    );
    assert_eq!(
        Cli::try_parse_from(["tool", "serve", "--port", "443", "--https"]).unwrap(),
        serve(443, false, true, &[])
    );
}

#[test]
fn trailing_var_arg_captures_everything_after_double_dash() {
    let cli = Cli::try_parse_from([
        "tool", "serve", "--port", "9", "--", "--https", "check", "", "-x", "serve",
    ])
    .unwrap();

    assert_eq!(
        cli.command,
        serve(9, false, false, &["--https", "check", "", "-x", "serve"]).command
    );
}

#[test]
fn unknown_option_before_double_dash_fails() {
    let err = Cli::try_parse_from(["tool", "serve", "--port", "9", "--bogus"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    let msg = err.to_string();
    assert!(msg.contains("--bogus"), "{msg}");
    assert!(msg.contains("Usage: tool serve"), "{msg}");

    // A trailing `--` does not rescue an earlier unknown option
    let err = Cli::try_parse_from(["tool", "serve", "--port", "9", "--bogus", "--", "ok"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn serve_requires_port() {
    let err = Cli::try_parse_from(["tool", "serve"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    let msg = err.to_string();
    assert!(msg.contains("--port <PORT>"), "{msg}");
    assert!(msg.contains("Usage: tool serve"), "{msg}");
}

#[test]
fn port_rejects_out_of_range_and_non_numeric_values() {
    let err = Cli::try_parse_from(["tool", "serve", "--port", "0"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ValueValidation);
    assert!(err.to_string().contains("--port"), "{}", err);

    let err = Cli::try_parse_from(["tool", "serve", "--port", "65536"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ValueValidation);

    let err = Cli::try_parse_from(["tool", "serve", "--port", "abc"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ValueValidation);
}

#[test]
fn required_option_without_a_value_fails_as_missing_value() {
    let err = Cli::try_parse_from(["tool", "serve", "--port"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidValue);
    let msg = err.to_string();
    assert!(msg.contains("a value is required for"), "{msg}");
    assert!(msg.contains("--port"), "{msg}");
}

#[test]
fn protocol_flags_are_mutually_exclusive() {
    let err = Cli::try_parse_from(["tool", "serve", "--port", "9", "--http", "--https"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    let msg = err.to_string();
    assert!(msg.contains("--http"), "{msg}");
    assert!(msg.contains("--https"), "{msg}");
}

#[test]
fn non_repeatable_args_cannot_be_repeated() {
    let err = Cli::try_parse_from(["tool", "--config", "a.toml", "--config", "b.toml", "check"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    assert!(err.to_string().contains("--config"), "{}", err);

    let err = Cli::try_parse_from(["tool", "check", "--strict", "--strict"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
}

#[test]
fn unknown_subcommand_fails() {
    let err = Cli::try_parse_from(["tool", "frobnicate"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    assert!(err.to_string().contains("frobnicate"), "{}", err);
}

#[test]
fn options_at_the_wrong_level_fail() {
    // `serve`-only options are rejected at the root and in `check`
    let err = Cli::try_parse_from(["tool", "--port", "9", "check"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    assert!(err.to_string().contains("--port"), "{}", err);

    let err = Cli::try_parse_from(["tool", "check", "--port", "9"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);

    let err = Cli::try_parse_from(["tool", "check", "--http"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);

    // `check`-only options are rejected in `serve`
    let err = Cli::try_parse_from(["tool", "serve", "--port", "9", "--strict"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    assert!(err.to_string().contains("--strict"), "{}", err);
}

#[test]
fn root_help_succeeds_and_lists_shared_args_and_subcommands() {
    let err = Cli::try_parse_from(["tool", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let msg = err.to_string();
    assert!(!msg.contains("error"), "{msg}");
    assert!(msg.contains("serve"), "{msg}");
    assert!(msg.contains("check"), "{msg}");
    assert!(msg.contains("--config"), "{msg}");
    assert!(msg.contains("--define"), "{msg}");
    assert!(msg.contains("builtin.toml"), "{msg}");
    // Subcommand-only args do not leak into root help
    assert!(!msg.contains("--port"), "{msg}");
    assert!(!msg.contains("--strict"), "{msg}");
}

#[test]
fn serve_help_succeeds_without_required_port() {
    let err = Cli::try_parse_from(["tool", "serve", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let msg = err.to_string();
    assert!(!msg.contains("error"), "{msg}");
    assert!(msg.contains("--port"), "{msg}");
    assert!(msg.contains("--http"), "{msg}");
    assert!(msg.contains("--https"), "{msg}");
    // Usage is stable and names the most specific command
    assert!(msg.contains("Usage: tool serve"), "{msg}");
    // Shared global args remain visible; `check`-only args do not appear
    assert!(msg.contains("--config"), "{msg}");
    assert!(!msg.contains("--strict"), "{msg}");
}

#[test]
fn check_help_lists_only_check_and_shared_args() {
    let err = Cli::try_parse_from(["tool", "check", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    let msg = err.to_string();
    assert!(!msg.contains("error"), "{msg}");
    assert!(msg.contains("--strict"), "{msg}");
    assert!(msg.contains("--config"), "{msg}");
    assert!(!msg.contains("--port"), "{msg}");
    assert!(!msg.contains("--http"), "{msg}");
}

#[test]
fn parsing_is_reproducible_after_a_failure() {
    const VALID: [&str; 7] = [
        "tool", "--define", "k=v", "serve", "--port", "443", "--https",
    ];

    let first = Cli::try_parse_from(VALID).unwrap();
    let first_help = Cli::try_parse_from(["tool", "serve", "--help"])
        .unwrap_err()
        .to_string();

    // Interleave failures of every category
    assert!(Cli::try_parse_from(["tool", "serve"]).is_err());
    assert!(Cli::try_parse_from(["tool", "serve", "--port", "0"]).is_err());
    assert!(Cli::try_parse_from(["tool", "serve", "--port", "9", "--http", "--https"]).is_err());
    assert!(Cli::try_parse_from(["tool", "frobnicate"]).is_err());
    assert!(Cli::try_parse_from(["tool", "check", "--bogus"]).is_err());

    let after = Cli::try_parse_from(VALID).unwrap();
    assert_eq!(first, after);

    let after_help = Cli::try_parse_from(["tool", "serve", "--help"])
        .unwrap_err()
        .to_string();
    assert_eq!(first_help, after_help);
}
