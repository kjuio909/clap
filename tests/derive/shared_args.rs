use clap::error::ErrorKind;
use clap::{Args, CommandFactory, Parser, Subcommand};
use snapbox::assert_data_eq;
use snapbox::str;

use crate::utils;

#[derive(Parser, Debug, PartialEq)]
#[command(name = "tool")]
struct Cli {
    #[command(flatten)]
    shared: Shared,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Args, Debug, PartialEq)]
struct Shared {
    #[arg(long, global = true, default_value = "builtin.toml")]
    config: String,
    #[arg(long, global = true, value_name = "key=value")]
    define: Vec<String>,
}

#[derive(Subcommand, Debug, PartialEq)]
enum Commands {
    Serve(Serve),
    Check(Check),
}

#[derive(Args, Debug, PartialEq)]
struct Serve {
    #[arg(long, value_parser = clap::value_parser!(u16).range(1..))]
    port: u16,
    /// Use plain HTTP (conflicts with --https)
    #[arg(long, conflicts_with = "https")]
    http: bool,
    /// Use HTTPS (conflicts with --http)
    #[arg(long)]
    https: bool,
    #[arg(last = true)]
    args: Vec<String>,
}

#[derive(Args, Debug, PartialEq)]
struct Check {
    path: Option<String>,
    #[arg(long)]
    strict: bool,
}

fn serve(port: u16, http: bool, https: bool, args: &[&str]) -> Cli {
    Cli {
        shared: Shared {
            config: "builtin.toml".to_owned(),
            define: Vec::new(),
        },
        command: Commands::Serve(Serve {
            port,
            http,
            https,
            args: args.iter().map(|s| (*s).to_owned()).collect(),
        }),
    }
}

#[test]
fn shared_args_before_subcommand() {
    let cli = Cli::try_parse_from([
        "tool",
        "--config",
        "app.toml",
        "--define",
        "a=1",
        "--define",
        "b=2",
        "serve",
        "--port",
        "8080",
    ])
    .unwrap();
    assert_eq!(
        cli,
        Cli {
            shared: Shared {
                config: "app.toml".to_owned(),
                define: vec!["a=1".to_owned(), "b=2".to_owned()],
            },
            command: Commands::Serve(Serve {
                port: 8080,
                http: false,
                https: false,
                args: Vec::new(),
            }),
        }
    );
}

#[test]
fn shared_args_after_subcommand() {
    let cli = Cli::try_parse_from([
        "tool",
        "serve",
        "--port",
        "8080",
        "--config",
        "app.toml",
        "--define",
        "a=1",
        "--define",
        "b=2",
    ])
    .unwrap();
    assert_eq!(
        cli,
        Cli {
            shared: Shared {
                config: "app.toml".to_owned(),
                define: vec!["a=1".to_owned(), "b=2".to_owned()],
            },
            command: Commands::Serve(Serve {
                port: 8080,
                http: false,
                https: false,
                args: Vec::new(),
            }),
        }
    );
}

#[test]
fn shared_args_split_around_subcommand_keep_order() {
    let before = Cli::try_parse_from([
        "tool",
        "--define",
        "a=1",
        "check",
        "--define",
        "b=2",
        "--define",
        "c=3",
    ])
    .unwrap();
    let after = Cli::try_parse_from([
        "tool", "check", "--define", "a=1", "--define", "b=2", "--define", "c=3",
    ])
    .unwrap();
    let expected = Cli {
        shared: Shared {
            config: "builtin.toml".to_owned(),
            define: vec!["a=1".to_owned(), "b=2".to_owned(), "c=3".to_owned()],
        },
        command: Commands::Check(Check {
            path: None,
            strict: false,
        }),
    };
    assert_eq!(before, expected);
    assert_eq!(after, expected);
}

#[test]
fn config_default_applies() {
    assert_eq!(
        Cli::try_parse_from(["tool", "serve", "--port", "1"]).unwrap(),
        serve(1, false, false, &[])
    );
    assert_eq!(
        Cli::try_parse_from(["tool", "check"]).unwrap(),
        Cli {
            shared: Shared {
                config: "builtin.toml".to_owned(),
                define: Vec::new(),
            },
            command: Commands::Check(Check {
                path: None,
                strict: false,
            }),
        }
    );
}

#[test]
fn serve_trailing_args_after_double_dash() {
    let cli = Cli::try_parse_from([
        "tool", "serve", "--port", "8080", "--", "--check", "check", "", "-x",
    ])
    .unwrap();
    assert_eq!(
        cli,
        serve(8080, false, false, &["--check", "check", "", "-x"])
    );
}

#[test]
fn serve_trailing_args_absent() {
    let cli = Cli::try_parse_from(["tool", "serve", "--port", "8080"]).unwrap();
    assert_eq!(cli, serve(8080, false, false, &[]));
}

#[test]
fn serve_unknown_option_before_double_dash_fails() {
    let err = Cli::try_parse_from(["tool", "serve", "--port", "8080", "--bogus"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn serve_protocol_flags() {
    assert_eq!(
        Cli::try_parse_from(["tool", "serve", "--port", "80", "--http"]).unwrap(),
        serve(80, true, false, &[])
    );
    assert_eq!(
        Cli::try_parse_from(["tool", "serve", "--port", "443", "--https"]).unwrap(),
        serve(443, false, true, &[])
    );
}

#[test]
fn serve_conflicting_protocols_fail() {
    let err = Cli::try_parse_from(["tool", "serve", "--port", "80", "--http", "--https"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
}

#[test]
fn serve_missing_port_fails() {
    let err = Cli::try_parse_from(["tool", "serve"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
}

#[test]
fn serve_port_out_of_range_fails() {
    for port in ["0", "65536"] {
        let err = Cli::try_parse_from(["tool", "serve", "--port", port]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ValueValidation, "port {port}");
    }
}

#[test]
fn serve_port_not_a_number_fails() {
    let err = Cli::try_parse_from(["tool", "serve", "--port", "http"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ValueValidation);
}

#[test]
fn serve_port_boundaries() {
    assert_eq!(
        Cli::try_parse_from(["tool", "serve", "--port", "1"]).unwrap(),
        serve(1, false, false, &[])
    );
    assert_eq!(
        Cli::try_parse_from(["tool", "serve", "--port", "65535"]).unwrap(),
        serve(65535, false, false, &[])
    );
}

#[test]
fn check_args() {
    assert_eq!(
        Cli::try_parse_from(["tool", "check", "src/", "--strict"]).unwrap(),
        Cli {
            shared: Shared {
                config: "builtin.toml".to_owned(),
                define: Vec::new(),
            },
            command: Commands::Check(Check {
                path: Some("src/".to_owned()),
                strict: true,
            }),
        }
    );
}

#[test]
fn check_rejects_serve_only_args() {
    let err = Cli::try_parse_from(["tool", "check", "--port", "1"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    let err = Cli::try_parse_from(["tool", "check", "--http"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn serve_rejects_check_only_args() {
    let err = Cli::try_parse_from(["tool", "serve", "--port", "1", "--strict"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn root_rejects_subcommand_args() {
    let err = Cli::try_parse_from(["tool", "--port", "1", "serve"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    let err = Cli::try_parse_from(["tool", "--strict", "check"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
}

#[test]
fn repeated_non_repeatable_arg_fails() {
    let err = Cli::try_parse_from([
        "tool", "--config", "a.toml", "--config", "b.toml", "check",
    ])
    .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
}

#[test]
fn unknown_subcommand_fails() {
    let err = Cli::try_parse_from(["tool", "frobnicate"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
}

#[test]
fn missing_subcommand_fails() {
    let err = Cli::try_parse_from(["tool"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand);
}

#[test]
fn root_help_succeeds_without_required_args() {
    let err = Cli::try_parse_from(["tool", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    assert_data_eq!(err.render().to_string(), str![[r#"
Usage: tool [OPTIONS] <COMMAND>

Commands:
  serve  
  check  
  help   Print this message or the help of the given subcommand(s)

Options:
      --config <CONFIG>     [default: builtin.toml]
      --define <key=value>  
  -h, --help                Print help

"#]]);
}

#[test]
fn serve_help_succeeds_without_required_port() {
    let err = Cli::try_parse_from(["tool", "serve", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    assert_data_eq!(err.render().to_string(), str![[r#"
Usage: tool serve [OPTIONS] --port <PORT> [-- <ARGS>...]

Arguments:
  [ARGS]...  

Options:
      --config <CONFIG>     [default: builtin.toml]
      --port <PORT>         
      --define <key=value>  
      --http                Use plain HTTP (conflicts with --https)
      --https               Use HTTPS (conflicts with --http)
  -h, --help                Print help

"#]]);
}

#[test]
fn check_help_succeeds() {
    let err = Cli::try_parse_from(["tool", "check", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    assert_data_eq!(err.render().to_string(), str![[r#"
Usage: tool check [OPTIONS] [PATH]

Arguments:
  [PATH]  

Options:
      --config <CONFIG>     [default: builtin.toml]
      --strict              
      --define <key=value>  
  -h, --help                Print help

"#]]);
}

#[test]
fn help_via_command_factory_matches_parse_help() {
    let rendered = utils::get_help::<Cli>();
    let err = Cli::try_parse_from(["tool", "--help"]).unwrap_err();
    assert_eq!(rendered, err.render().to_string());
}

#[test]
fn parse_after_failure_is_unchanged() {
    // A failed parse must not leave observable state behind
    assert!(Cli::try_parse_from(["tool", "serve"]).is_err());

    let args = [
        "tool",
        "--define",
        "a=1",
        "serve",
        "--port",
        "8080",
        "--https",
        "--define",
        "b=2",
        "--",
        "--raw",
    ];
    let after_failure = Cli::try_parse_from(args).unwrap();
    let fresh = Cli::try_parse_from(args).unwrap();
    assert_eq!(after_failure, fresh);

    // Help after a failure must be identical to help on a fresh definition
    let help_after_failure = Cli::try_parse_from(["tool", "serve", "--help"]).unwrap_err();
    let help_fresh = Cli::try_parse_from(["tool", "serve", "--help"]).unwrap_err();
    assert_eq!(
        help_after_failure.render().to_string(),
        help_fresh.render().to_string()
    );
}

#[test]
fn command_debug_and_verify() {
    Cli::command().debug_assert();
}
