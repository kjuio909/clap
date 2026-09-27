//! Derived CLI definition covering nested subcommands, global arguments, and
//! failure recovery. Everything is observed through `Cli::try_parse_from`
//! with token sequences that include argv0.
#![allow(unreachable_pub)] // types stay observable to sibling test modules

use clap::Args;
use clap::Parser;
use clap::Subcommand;

fn parse_port(raw: &str) -> Result<u16, String> {
    let value: u64 = raw
        .parse()
        .map_err(|_| format!("invalid port `{raw}`: expected a decimal integer"))?;
    if (1..=65535).contains(&value) {
        Ok(value as u16)
    } else {
        Err(format!(
            "invalid port `{raw}`: must be between 1 and 65535"
        ))
    }
}

#[derive(Parser, Debug, PartialEq)]
#[command(
    name = "tool",
    about = "Root command with global options",
    override_usage = "tool [OPTIONS] <COMMAND>"
)]
pub struct Cli {
    #[arg(long, global = true, default_value = "builtin.toml", value_name = "path")]
    pub config: String,
    #[arg(long, global = true, value_name = "key=value")]
    pub define: Vec<String>,
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum Commands {
    #[command(about = "Run the server")]
    Serve(ServeArgs),
    #[command(about = "Check a path")]
    Check(CheckArgs),
}

#[derive(Args, Debug, PartialEq)]
#[command(override_usage = "tool serve [OPTIONS] <COMMAND>")]
pub struct ServeArgs {
    #[arg(long)]
    pub profile: Option<String>,
    #[command(subcommand)]
    pub command: ServeCommands,
}

#[derive(Subcommand, Debug, PartialEq)]
pub enum ServeCommands {
    #[command(about = "Run a worker")]
    Worker(WorkerArgs),
}

#[derive(Args, Debug, PartialEq)]
#[command(
    group = clap::ArgGroup::new("protocol").args(["http", "https"]).multiple(false),
    override_usage = "tool serve worker [OPTIONS] --port <PORT> [-- <TRAILING>...]"
)]
pub struct WorkerArgs {
    #[arg(long, value_parser = parse_port)]
    pub port: u16,
    #[arg(long, help = "Serve plain HTTP (mutually exclusive with --https)")]
    pub http: bool,
    #[arg(long, help = "Serve HTTPS (mutually exclusive with --http)")]
    pub https: bool,
    #[arg(last = true)]
    pub trailing: Vec<String>,
}

#[derive(Args, Debug, PartialEq)]
#[command(override_usage = "tool check [OPTIONS] [path]")]
pub struct CheckArgs {
    pub path: Option<String>,
    #[arg(long)]
    pub strict: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::error::ErrorKind;

    fn parse(argv: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(argv)
    }

    fn worker_ok(argv: &[&str]) -> (Cli, WorkerArgs) {
        let cli = parse(argv).unwrap();
        let Cli {
            command: Commands::Serve(ServeArgs {
                command: ServeCommands::Worker(ref worker),
                ..
            }),
            ..
        } = cli
        else {
            panic!("expected `serve worker` parse");
        };
        let worker = WorkerArgs {
            port: worker.port,
            http: worker.http,
            https: worker.https,
            trailing: worker.trailing.clone(),
        };
        (cli, worker)
    }

    #[test]
    fn minimal_worker_parse() {
        let (cli, worker) = worker_ok(&["tool", "serve", "worker", "--port", "8080"]);
        assert_eq!(cli.config, "builtin.toml");
        assert_eq!(cli.define, Vec::<String>::new());
        assert_eq!(worker.port, 8080);
        assert!(!worker.http && !worker.https);
        assert_eq!(worker.trailing, Vec::<String>::new());
    }

    #[test]
    fn explicit_config_overrides_default() {
        let (cli, _) = worker_ok(&[
            "tool", "--config", "custom.toml", "serve", "worker", "--port", "1",
        ]);
        assert_eq!(cli.config, "custom.toml");
    }

    #[test]
    fn defines_preserve_order() {
        let (cli, _) = worker_ok(&[
            "tool",
            "--define",
            "b=2",
            "serve",
            "--define",
            "a=1",
            "worker",
            "--port",
            "1",
            "--define",
            "c=3",
        ]);
        assert_eq!(cli.define, vec!["b=2", "a=1", "c=3"]);
    }

    #[test]
    fn global_args_accepted_at_any_position() {
        let expected = worker_ok(&[
            "tool", "--config", "x.toml", "--define", "k=v", "serve", "worker", "--port", "9",
        ]);
        for argv in [
            &["tool", "serve", "--config", "x.toml", "worker", "--define", "k=v", "--port", "9"][..],
            &["tool", "serve", "worker", "--port", "9", "--config", "x.toml", "--define", "k=v"][..],
            &["tool", "serve", "worker", "--config", "x.toml", "--port", "9", "--define", "k=v"][..],
        ] {
            assert_eq!(parse(argv).unwrap(), expected.0);
        }
    }

    #[test]
    fn port_bounds() {
        assert_eq!(worker_ok(&["tool", "serve", "worker", "--port", "1"]).1.port, 1);
        assert_eq!(
            worker_ok(&["tool", "serve", "worker", "--port", "65535"]).1.port,
            65535
        );
    }

    #[test]
    fn protocol_flags_are_exclusive() {
        let (_, worker) = worker_ok(&["tool", "serve", "worker", "--port", "1", "--http"]);
        assert!(worker.http && !worker.https);
        let (_, worker) = worker_ok(&["tool", "serve", "worker", "--port", "1", "--https"]);
        assert!(worker.https && !worker.http);
    }

    #[test]
    fn trailing_tokens_after_double_dash_are_raw() {
        let (cli, worker) = worker_ok(&[
            "tool", "serve", "worker", "--port", "1", "--", "--config", "serve", "check",
            "--http", "--",
        ]);
        assert_eq!(
            worker.trailing,
            vec!["--config", "serve", "check", "--http", "--"]
        );
        // Tokens after `--` must not be interpreted as global args.
        assert_eq!(cli.config, "builtin.toml");
    }

    #[test]
    fn unknown_token_before_separator_fails_at_worker() {
        let err = parse(&["tool", "serve", "worker", "--port", "1", "--bogus"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        let msg = err.to_string();
        assert!(msg.contains("--bogus"), "{msg}");
        assert!(msg.contains("tool serve worker"), "{msg}");
    }

    #[test]
    fn check_parses_optional_path_and_strict() {
        let cli = parse(&["tool", "check"]).unwrap();
        let Commands::Check(check) = cli.command else {
            panic!("expected check");
        };
        assert_eq!(check.path, None);
        assert!(!check.strict);

        let cli = parse(&["tool", "check", "some/path", "--strict"]).unwrap();
        let Commands::Check(check) = cli.command else {
            panic!("expected check");
        };
        assert_eq!(check.path.as_deref(), Some("some/path"));
        assert!(check.strict);
    }

    #[test]
    fn check_treats_post_separator_token_as_path() {
        let cli = parse(&["tool", "check", "--", "--strict"]).unwrap();
        let Commands::Check(check) = cli.command else {
            panic!("expected check");
        };
        assert_eq!(check.path.as_deref(), Some("--strict"));
        assert!(!check.strict);

        let err = parse(&["tool", "check", "--", "a", "b"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    }

    #[test]
    fn check_rejects_serve_and_worker_args() {
        for argv in [
            &["tool", "check", "--profile", "p"][..],
            &["tool", "check", "--port", "1"][..],
            &["tool", "check", "--http"][..],
        ] {
            let err = parse(argv).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::UnknownArgument, "{argv:?}");
        }
    }

    #[test]
    fn missing_port_fails() {
        let err = parse(&["tool", "serve", "worker"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
        let msg = err.to_string();
        assert!(msg.contains("--port"), "{msg}");
        assert!(msg.contains("tool serve worker"), "{msg}");
    }

    #[test]
    fn invalid_ports_fail() {
        for bad in ["0", "65536", "abc", "1.5", "0x10"] {
            let err = parse(&["tool", "serve", "worker", "--port", bad]).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::ValueValidation, "{bad}");
            assert!(err.to_string().contains(bad), "{bad}");
        }
        // A leading hyphen makes the token an unknown flag rather than a value.
        assert!(parse(&["tool", "serve", "worker", "--port", "-1"]).is_err());
        let err = parse(&["tool", "serve", "worker", "--port=-1"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ValueValidation);
    }

    #[test]
    fn conflicting_protocols_fail() {
        let err = parse(&["tool", "serve", "worker", "--port", "1", "--http", "--https"])
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    }

    #[test]
    fn duplicate_values_fail() {
        for argv in [
            &["tool", "serve", "worker", "--port", "1", "--port", "2"][..],
            &["tool", "serve", "--profile", "a", "--profile", "b", "worker", "--port", "1"][..],
            &["tool", "serve", "worker", "--port", "1", "--http", "--http"][..],
        ] {
            assert!(parse(argv).is_err(), "{argv:?}");
        }
    }

    #[test]
    fn unknown_subcommand_fails() {
        let err = parse(&["tool", "serv"]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
        assert!(err.to_string().contains("serv"));
    }

    #[test]
    fn missing_option_value_fails() {
        for argv in [
            &["tool", "--config"][..],
            &["tool", "serve", "--profile"][..],
            &["tool", "serve", "worker", "--port"][..],
            &["tool", "--define"][..],
        ] {
            let err = parse(argv).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::InvalidValue, "{argv:?}");
        }
    }

    #[test]
    fn failure_does_not_pollute_next_parse() {
        assert!(parse(&["tool", "serve", "worker"]).is_err());
        assert!(parse(&["tool", "serve", "worker", "--port", "abc"]).is_err());
        let (cli, worker) = worker_ok(&["tool", "serve", "worker", "--port", "80"]);
        assert_eq!(cli.config, "builtin.toml");
        assert_eq!(worker.port, 80);
    }

    #[test]
    fn help_succeeds_at_every_level() {
        let root = parse(&["tool", "--help"]).unwrap_err();
        assert_eq!(root.kind(), ErrorKind::DisplayHelp);
        let root = root.to_string();
        for needle in ["--config", "--define", "builtin.toml", "serve", "check"] {
            assert!(root.contains(needle), "root help missing {needle}:\n{root}");
        }
        assert!(!root.contains("--profile"), "{root}");
        assert!(!root.contains("--port"), "{root}");

        let serve = parse(&["tool", "serve", "--help"]).unwrap_err();
        assert_eq!(serve.kind(), ErrorKind::DisplayHelp);
        let serve = serve.to_string();
        for needle in ["--profile", "worker", "--config", "--define"] {
            assert!(serve.contains(needle), "serve help missing {needle}:\n{serve}");
        }
        assert!(!serve.contains("--port"), "{serve}");
        assert!(!serve.contains("--strict"), "{serve}");

        // Required `--port` is missing, yet help must still succeed.
        let worker = parse(&["tool", "serve", "worker", "--help"]).unwrap_err();
        assert_eq!(worker.kind(), ErrorKind::DisplayHelp);
        let worker = worker.to_string();
        for needle in [
            "--port",
            "--http",
            "--https",
            "mutually exclusive with --https",
            "mutually exclusive with --http",
            "--config",
        ] {
            assert!(worker.contains(needle), "worker help missing {needle}:\n{worker}");
        }
        assert!(!worker.contains("--profile"), "{worker}");
        assert!(!worker.contains("--strict"), "{worker}");

        let check = parse(&["tool", "check", "--help"]).unwrap_err();
        assert_eq!(check.kind(), ErrorKind::DisplayHelp);
        let check = check.to_string();
        for needle in ["--strict", "--config", "--define"] {
            assert!(check.contains(needle), "check help missing {needle}:\n{check}");
        }
        for absent in ["--profile", "--port", "--http"] {
            assert!(!check.contains(absent), "check help leaks {absent}:\n{check}");
        }
    }

    #[test]
    fn global_position_swap_keeps_help_and_errors_stable() {
        let baseline = parse(&["tool", "--config", "x.toml", "serve", "worker", "--help"])
            .unwrap_err()
            .to_string();
        let swapped = parse(&["tool", "serve", "worker", "--config", "x.toml", "--help"])
            .unwrap_err()
            .to_string();
        assert_eq!(baseline, swapped);

        let baseline = parse(&["tool", "--config", "x.toml", "serve", "worker"]).unwrap_err();
        let swapped = parse(&["tool", "serve", "worker", "--config", "x.toml"]).unwrap_err();
        assert_eq!(baseline.kind(), swapped.kind());
        assert_eq!(baseline.to_string(), swapped.to_string());
    }
}
