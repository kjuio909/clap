use clap::{error::ErrorKind, Arg, ArgAction, Command};

fn tool_command() -> Command {
    Command::new("tool")
        .about("A test CLI")
        .subcommand_required(true)
        .subcommand(
            Command::new("run")
                .about("Run the tool")
                .arg(
                    Arg::new("output")
                        .short('o')
                        .long("output")
                        .value_name("PATH")
                        .help("Write results to PATH")
                        .required(true),
                )
                .arg(
                    Arg::new("json")
                        .long("json")
                        .action(ArgAction::SetTrue)
                        .help("Emit JSON output")
                        .conflicts_with("yaml"),
                )
                .arg(
                    Arg::new("yaml")
                        .long("yaml")
                        .action(ArgAction::SetTrue)
                        .help("Emit YAML output"),
                ),
        )
        .subcommand(
            Command::new("config")
                .about("Configure the tool")
                .subcommand_required(true)
                .subcommand(
                    Command::new("set")
                        .about("Set a configuration value")
                        .arg(Arg::new("key").help("The key to set").required(true))
                        .arg(Arg::new("value").help("The value to set").required(true)),
                )
                .subcommand(Command::new("show").about("Show the configuration")),
        )
}

#[test]
fn run_parses_with_required_output() {
    let m = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--output", "out.txt"])
        .unwrap();
    let (name, sub_m) = m.subcommand().unwrap();
    assert_eq!(name, "run");
    assert_eq!(
        sub_m.get_one::<String>("output").map(String::as_str),
        Some("out.txt")
    );
    assert_eq!(sub_m.get_flag("json"), false);
    assert_eq!(sub_m.get_flag("yaml"), false);
}

#[test]
fn run_parse_result_is_independent_of_option_order() {
    let forward = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--json", "--output", "out.txt"])
        .unwrap();
    let reversed = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--output", "out.txt", "--json"])
        .unwrap();

    for m in [forward, reversed] {
        let (name, sub_m) = m.subcommand().unwrap();
        assert_eq!(name, "run");
        assert_eq!(
            sub_m.get_one::<String>("output").map(String::as_str),
            Some("out.txt")
        );
        assert!(sub_m.get_flag("json"));
        assert_eq!(sub_m.get_flag("yaml"), false);
    }
}

#[test]
fn run_parses_with_short_output_flag() {
    let m = tool_command()
        .try_get_matches_from(vec!["tool", "run", "-o", "out.txt", "--yaml"])
        .unwrap();
    let (name, sub_m) = m.subcommand().unwrap();
    assert_eq!(name, "run");
    assert_eq!(
        sub_m.get_one::<String>("output").map(String::as_str),
        Some("out.txt")
    );
    assert!(sub_m.get_flag("yaml"));
    assert_eq!(sub_m.get_flag("json"), false);
}

#[test]
fn config_set_parses_key_and_value() {
    let m = tool_command()
        .try_get_matches_from(vec!["tool", "config", "set", "color", "always"])
        .unwrap();
    let (name, config_m) = m.subcommand().unwrap();
    assert_eq!(name, "config");
    let (name, set_m) = config_m.subcommand().unwrap();
    assert_eq!(name, "set");
    assert_eq!(
        set_m.get_one::<String>("key").map(String::as_str),
        Some("color")
    );
    assert_eq!(
        set_m.get_one::<String>("value").map(String::as_str),
        Some("always")
    );
}

#[test]
fn config_show_parses() {
    let m = tool_command()
        .try_get_matches_from(vec!["tool", "config", "show"])
        .unwrap();
    let (name, config_m) = m.subcommand().unwrap();
    assert_eq!(name, "config");
    assert_eq!(config_m.subcommand_name(), Some("show"));
}

#[test]
fn conflicting_format_flags_are_rejected() {
    for args in [
        vec!["tool", "run", "--output", "out.txt", "--json", "--yaml"],
        vec!["tool", "run", "--output", "out.txt", "--yaml", "--json"],
    ] {
        let err = tool_command().try_get_matches_from(args).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
        let msg = err.to_string();
        assert!(msg.contains("--json"), "{msg}");
        assert!(msg.contains("--yaml"), "{msg}");
        assert!(msg.contains("Usage: tool run"), "{msg}");
        assert!(err.use_stderr());
        assert_ne!(err.exit_code(), 0);
    }
}

#[test]
fn duplicate_output_option_is_rejected() {
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--output", "a", "--output", "b"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    let msg = err.to_string();
    assert!(msg.contains("--output"), "{msg}");
    assert!(msg.contains("cannot be used multiple times"), "{msg}");
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);
}

#[test]
fn missing_output_is_rejected_as_missing_required() {
    for args in [
        vec!["tool", "run"],
        vec!["tool", "run", "--json"],
        vec!["tool", "run", "--yaml"],
    ] {
        let err = tool_command().try_get_matches_from(args).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
        let msg = err.to_string();
        assert!(msg.contains("--output <PATH>"), "{msg}");
        assert!(msg.contains("Usage: tool run"), "{msg}");
        assert!(err.use_stderr());
        assert_ne!(err.exit_code(), 0);
    }
}

#[test]
fn option_present_without_value_is_distinct_from_missing_option() {
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--output"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidValue);
    assert_ne!(err.kind(), ErrorKind::MissingRequiredArgument);
    let msg = err.to_string();
    assert!(msg.contains("--output <PATH>"), "{msg}");
    assert!(msg.contains("none was supplied"), "{msg}");
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);
}

#[test]
fn option_value_is_not_taken_from_following_flag() {
    // `--json` must not be consumed as the value of `--output`
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--output", "--json"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidValue);
    let msg = err.to_string();
    assert!(msg.contains("--output <PATH>"), "{msg}");
    assert!(err.use_stderr());
}

#[test]
fn foreign_subcommand_after_run_is_rejected() {
    // `set` belongs to `config`, not `run`; it must not fall back to the root
    // command or be silently ignored.
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--output", "out.txt", "set"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    let msg = err.to_string();
    assert!(msg.contains("set"), "{msg}");
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);
}

#[test]
fn unknown_subcommand_under_config_is_rejected() {
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "config", "list"])
        .unwrap_err();
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);
    let msg = err.to_string();
    assert!(msg.contains("list"), "{msg}");
}

#[test]
fn extra_positional_after_run_is_rejected() {
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--output", "out.txt", "extra"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    let msg = err.to_string();
    assert!(msg.contains("extra"), "{msg}");
    assert!(err.use_stderr());
}

#[test]
fn stray_separator_does_not_smuggle_tokens_into_root() {
    // After `--`, `run` is a positional value, but the root command accepts
    // none, so this must fail rather than dispatch to the `run` subcommand.
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "--", "run"])
        .unwrap_err();
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);
}

#[test]
fn config_set_requires_key_and_value() {
    for args in [
        vec!["tool", "config", "set"],
        vec!["tool", "config", "set", "color"],
    ] {
        let err = tool_command().try_get_matches_from(args).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
        assert!(err.use_stderr());
        assert_ne!(err.exit_code(), 0);
    }
}

#[test]
fn root_help_succeeds_without_subcommand() {
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "--help"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    assert!(!err.use_stderr());
    assert_eq!(err.exit_code(), 0);
    let msg = err.to_string();
    assert!(!msg.contains("error:"), "{msg}");
    assert!(msg.contains("run"), "{msg}");
    assert!(msg.contains("config"), "{msg}");
    assert!(msg.contains("Usage:"), "{msg}");
}

#[test]
fn run_help_succeeds_without_required_output() {
    for flag in ["--help", "-h"] {
        let err = tool_command()
            .try_get_matches_from(vec!["tool", "run", flag])
            .unwrap_err();
        assert_eq!(err.kind(), ErrorKind::DisplayHelp);
        assert!(!err.use_stderr());
        assert_eq!(err.exit_code(), 0);
        let msg = err.to_string();
        assert!(!msg.contains("error:"), "{msg}");
        assert!(msg.contains("--output"), "{msg}");
        assert!(msg.contains("--json"), "{msg}");
        assert!(msg.contains("--yaml"), "{msg}");
        assert!(msg.contains("Usage: tool run"), "{msg}");
    }
}

#[test]
fn help_lists_options_in_definition_order() {
    let err = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--help"])
        .unwrap_err();
    let msg = err.to_string();
    let output = msg.find("--output").unwrap();
    let json = msg.find("--json").unwrap();
    let yaml = msg.find("--yaml").unwrap();
    assert!(output < json, "{msg}");
    assert!(json < yaml, "{msg}");
}

#[test]
fn failed_parse_does_not_pollute_command_definition() {
    let mut cmd = tool_command();

    // A failed parse must not leave partial state behind.
    let err = cmd
        .try_get_matches_from_mut(vec!["tool", "run", "--output", "out.txt", "--json", "--yaml"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);

    // The same command definition still parses valid input.
    let m = cmd
        .try_get_matches_from_mut(vec!["tool", "run", "--output", "out.txt", "--json"])
        .unwrap();
    let (name, sub_m) = m.subcommand().unwrap();
    assert_eq!(name, "run");
    assert_eq!(
        sub_m.get_one::<String>("output").map(String::as_str),
        Some("out.txt")
    );
    assert!(sub_m.get_flag("json"));

    // Sibling subcommands are unaffected.
    let m = cmd
        .try_get_matches_from_mut(vec!["tool", "config", "set", "color", "always"])
        .unwrap();
    assert_eq!(m.subcommand_name(), Some("config"));

    // Help output is still available and identical to a fresh command's.
    let reused = cmd
        .try_get_matches_from_mut(vec!["tool", "run", "--help"])
        .unwrap_err()
        .to_string();
    let fresh = tool_command()
        .try_get_matches_from(vec!["tool", "run", "--help"])
        .unwrap_err()
        .to_string();
    assert_eq!(reused, fresh);
}

#[test]
fn error_and_help_output_is_stable_across_parses() {
    let first = tool_command()
        .try_get_matches_from(vec!["tool", "run"])
        .unwrap_err()
        .to_string();
    let second = tool_command()
        .try_get_matches_from(vec!["tool", "run"])
        .unwrap_err()
        .to_string();
    assert_eq!(first, second);

    let first = tool_command()
        .try_get_matches_from(vec!["tool", "--help"])
        .unwrap_err()
        .to_string();
    let second = tool_command()
        .try_get_matches_from(vec!["tool", "--help"])
        .unwrap_err()
        .to_string();
    assert_eq!(first, second);
}
