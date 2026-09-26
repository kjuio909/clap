#![cfg(feature = "unstable-dynamic")]

use std::ffi::OsString;

/// `tool` with a `deploy` subcommand (alias `d`):
/// - `--format` / `-f` with values `json`, `yaml`
/// - repeatable `--profile` with values `dev`, `prod`
/// - positional `target` with values `all`, `changed`
fn tool_cmd() -> clap::Command {
    clap::Command::new("tool")
        .disable_help_flag(true)
        .disable_help_subcommand(true)
        .subcommand(
            clap::Command::new("deploy")
                .alias("d")
                .arg(
                    clap::Arg::new("format")
                        .long("format")
                        .short('f')
                        .value_parser(["json", "yaml"]),
                )
                .arg(
                    clap::Arg::new("profile")
                        .long("profile")
                        .action(clap::ArgAction::Append)
                        .value_parser(["dev", "prod"]),
                )
                .arg(clap::Arg::new("target").value_parser(["all", "changed"])),
        )
}

/// Complete with the cursor on the last token of `args` (which includes argv0).
fn complete(args: &[&str]) -> Vec<String> {
    let mut cmd = tool_cmd();
    let argv: Vec<OsString> = args.iter().map(OsString::from).collect();
    let arg_index = argv.len() - 1;
    clap_complete::engine::complete(&mut cmd, argv, arg_index, None)
        .unwrap()
        .into_iter()
        .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn separated_long_value() {
    assert_eq!(complete(&["tool", "deploy", "--format", ""]), ["json", "yaml"]);
    assert_eq!(complete(&["tool", "deploy", "--format", "j"]), ["json"]);
}

#[test]
fn equals_long_value() {
    assert_eq!(
        complete(&["tool", "deploy", "--format="]),
        ["--format=json", "--format=yaml"]
    );
    assert_eq!(complete(&["tool", "deploy", "--format=j"]), ["--format=json"]);
}

#[test]
fn short_value() {
    assert_eq!(complete(&["tool", "deploy", "-f", ""]), ["json", "yaml"]);
    assert_eq!(complete(&["tool", "deploy", "-fj"]), ["-fjson"]);
}

#[test]
fn alias_matches_subcommand() {
    for args in [
        &["--format", ""][..],
        &["--format", "j"][..],
        &["--format="][..],
        &["--format=j"][..],
        &["-f", ""][..],
        &["-fj"][..],
    ] {
        let mut deploy = vec!["tool", "deploy"];
        deploy.extend_from_slice(args);
        let mut alias = vec!["tool", "d"];
        alias.extend_from_slice(args);
        assert_eq!(complete(&deploy), complete(&alias), "args: {args:?}");
    }
}

#[test]
fn repeated_option_value() {
    assert_eq!(
        complete(&["tool", "deploy", "--profile", "dev", "--profile", ""]),
        ["dev", "prod"]
    );
}

#[test]
fn positional_after_escape() {
    assert_eq!(complete(&["tool", "d", "--", ""]), ["all", "changed"]);
    assert_eq!(
        complete(&["tool", "deploy", "--format", "json", "--", ""]),
        ["all", "changed"]
    );
}

#[test]
fn invalid_tokens_yield_no_candidates() {
    // Unknown option
    assert_eq!(complete(&["tool", "deploy", "--unknown"]), Vec::<String>::new());
    // Unknown option, already accepted, completing the next token
    assert_eq!(complete(&["tool", "deploy", "--unknown", ""]), Vec::<String>::new());
    // Unknown command
    assert_eq!(complete(&["tool", "bogus"]), Vec::<String>::new());
    // Unknown command, already accepted, completing the next token
    assert_eq!(complete(&["tool", "bogus", ""]), Vec::<String>::new());
    // Invalid value prefix
    assert_eq!(complete(&["tool", "deploy", "--format", "x"]), Vec::<String>::new());
    // Incomplete / invalid tokens
    assert_eq!(complete(&["tool", "deploy", "--format=--"]), Vec::<String>::new());
    assert_eq!(complete(&["tool", "deploy", "-x"]), Vec::<String>::new());
}

#[test]
fn root_empty_prefix() {
    assert_eq!(complete(&["tool", ""]), ["deploy"]);
}
