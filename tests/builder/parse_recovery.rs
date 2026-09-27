use clap::{error::ErrorKind, Arg, ArgAction, ArgMatches, Command};

fn tool() -> Command {
    Command::new("tool")
        .about("A CLI with predictable parse failures")
        .subcommand_required(true)
        .subcommand(
            Command::new("run")
                .about("Run the tool")
                .arg(
                    Arg::new("output")
                        .long("output")
                        .short('o')
                        .value_name("PATH")
                        .required(true)
                        .help("Path to write the output to"),
                )
                .arg(
                    Arg::new("json")
                        .long("json")
                        .action(ArgAction::SetTrue)
                        .conflicts_with("yaml")
                        .help("Emit JSON"),
                )
                .arg(
                    Arg::new("yaml")
                        .long("yaml")
                        .action(ArgAction::SetTrue)
                        .help("Emit YAML"),
                ),
        )
        .subcommand(
            Command::new("config")
                .about("Manage configuration")
                .subcommand_required(true)
                .subcommand(
                    Command::new("set")
                        .about("Set a configuration key")
                        .arg(Arg::new("key").required(true))
                        .arg(Arg::new("value").required(true)),
                )
                .subcommand(Command::new("show").about("Show the configuration")),
        )
}

fn run_matches(matches: &ArgMatches) -> &ArgMatches {
    match matches.subcommand() {
        Some(("run", sub)) => sub,
        other => panic!("expected `run` subcommand, got {other:?}"),
    }
}

#[test]
fn run_parses_valid_input() {
    let matches = tool()
        .try_get_matches_from(["tool", "run", "--output", "out.txt"])
        .unwrap();
    let run = run_matches(&matches);
    assert_eq!(
        run.get_one::<String>("output").map(String::as_str),
        Some("out.txt")
    );
    assert!(!run.get_flag("json"));
    assert!(!run.get_flag("yaml"));
}

#[test]
fn run_parses_each_format_flag() {
    let matches = tool()
        .try_get_matches_from(["tool", "run", "--output", "out.txt", "--json"])
        .unwrap();
    let run = run_matches(&matches);
    assert!(run.get_flag("json"));
    assert!(!run.get_flag("yaml"));

    let matches = tool()
        .try_get_matches_from(["tool", "run", "--output", "out.txt", "--yaml"])
        .unwrap();
    let run = run_matches(&matches);
    assert!(!run.get_flag("json"));
    assert!(run.get_flag("yaml"));
}

#[test]
fn run_option_order_does_not_change_matches() {
    let flag_first = tool().try_get_matches_from(["tool", "run", "--json", "--output", "out.txt"]);
    let flag_last = tool().try_get_matches_from(["tool", "run", "--output", "out.txt", "--json"]);
    let equals = tool().try_get_matches_from(["tool", "run", "--output=out.txt", "--json"]);
    let short = tool().try_get_matches_from(["tool", "run", "-o", "out.txt", "--json"]);

    for result in [flag_first, flag_last, equals, short] {
        let matches = result.unwrap();
        let run = run_matches(&matches);
        assert_eq!(
            run.get_one::<String>("output").map(String::as_str),
            Some("out.txt")
        );
        assert!(run.get_flag("json"));
        assert!(!run.get_flag("yaml"));
    }
}

#[test]
fn config_parses_set_and_show() {
    let matches = tool()
        .try_get_matches_from(["tool", "config", "set", "color", "always"])
        .unwrap();
    match matches.subcommand() {
        Some(("config", config)) => match config.subcommand() {
            Some(("set", set)) => {
                assert_eq!(
                    set.get_one::<String>("key").map(String::as_str),
                    Some("color")
                );
                assert_eq!(
                    set.get_one::<String>("value").map(String::as_str),
                    Some("always")
                );
            }
            other => panic!("expected `set` subcommand, got {other:?}"),
        },
        other => panic!("expected `config` subcommand, got {other:?}"),
    }

    let matches = tool()
        .try_get_matches_from(["tool", "config", "show"])
        .unwrap();
    match matches.subcommand() {
        Some(("config", config)) => {
            assert!(matches!(config.subcommand(), Some(("show", _))));
        }
        other => panic!("expected `config` subcommand, got {other:?}"),
    }
}

#[test]
fn conflicting_format_flags_are_rejected_in_either_order() {
    for args in [
        ["tool", "run", "--output", "o", "--json", "--yaml"],
        ["tool", "run", "--output", "o", "--yaml", "--json"],
    ] {
        let err = tool().try_get_matches_from(args).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
        assert!(err.use_stderr());
        assert_ne!(err.exit_code(), 0);

        let rendered = err.to_string();
        assert!(rendered.contains("--json"), "{rendered}");
        assert!(rendered.contains("--yaml"), "{rendered}");
        assert!(rendered.contains("Usage: tool run"), "{rendered}");
    }
}

#[test]
fn repeated_option_is_rejected() {
    let err = tool()
        .try_get_matches_from(["tool", "run", "--output", "a", "--output", "b"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);

    let rendered = err.to_string();
    assert!(rendered.contains("--output <PATH>"), "{rendered}");
    assert!(rendered.contains("Usage: tool run"), "{rendered}");
}

#[test]
fn missing_output_is_a_required_argument_error() {
    let err = tool().try_get_matches_from(["tool", "run"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);

    let rendered = err.to_string();
    assert!(rendered.contains("--output <PATH>"), "{rendered}");
    assert!(rendered.contains("Usage: tool run"), "{rendered}");
}

#[test]
fn option_without_value_is_distinct_from_missing_option() {
    for args in [
        &["tool", "run", "--output"][..],
        // The next token is a flag, not a value, and must not be consumed as one
        &["tool", "run", "--output", "--json"][..],
    ] {
        let err = tool().try_get_matches_from(args).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::InvalidValue);
        assert_ne!(err.kind(), ErrorKind::MissingRequiredArgument);
        assert!(err.use_stderr());
        assert_ne!(err.exit_code(), 0);

        let rendered = err.to_string();
        assert!(rendered.contains("--output <PATH>"), "{rendered}");
    }
}

#[test]
fn foreign_subcommand_after_run_is_rejected() {
    // `set` and `config` only exist at other levels of the tree; they must not
    // be silently accepted by (or fall back to) another command.
    for (args, token) in [
        (&["tool", "run", "--output", "o", "set"][..], "set"),
        (&["tool", "run", "config"][..], "config"),
    ] {
        let err = tool().try_get_matches_from(args).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::UnknownArgument);
        assert!(err.use_stderr());
        assert_ne!(err.exit_code(), 0);

        let rendered = err.to_string();
        assert!(rendered.contains(token), "{rendered}");
        assert!(rendered.contains("Usage: tool run"), "{rendered}");
    }
}

#[test]
fn config_rejects_unknown_subcommand() {
    let err = tool()
        .try_get_matches_from(["tool", "config", "bad"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::InvalidSubcommand);
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);

    let rendered = err.to_string();
    assert!(rendered.contains("bad"), "{rendered}");
    assert!(rendered.contains("Usage: tool config"), "{rendered}");
}

#[test]
fn config_requires_a_subcommand() {
    let err = tool().try_get_matches_from(["tool", "config"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::MissingSubcommand);
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);

    let rendered = err.to_string();
    assert!(rendered.contains("set"), "{rendered}");
    assert!(rendered.contains("show"), "{rendered}");
}

#[test]
fn config_set_requires_key_and_value() {
    let err = tool()
        .try_get_matches_from(["tool", "config", "set"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    let rendered = err.to_string();
    assert!(rendered.contains("<key>"), "{rendered}");
    assert!(rendered.contains("<value>"), "{rendered}");
    assert!(rendered.contains("Usage: tool config set"), "{rendered}");

    let err = tool()
        .try_get_matches_from(["tool", "config", "set", "color"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument);
    let rendered = err.to_string();
    assert!(rendered.contains("<value>"), "{rendered}");
    assert!(!rendered.contains("<key> was not"), "{rendered}");
}

#[test]
fn extra_tokens_fail_at_the_deepest_level() {
    let err = tool()
        .try_get_matches_from(["tool", "config", "set", "color", "always", "extra"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);

    let rendered = err.to_string();
    assert!(rendered.contains("extra"), "{rendered}");
    assert!(
        rendered.contains("Usage: tool config set <key> <value>"),
        "{rendered}"
    );
}

#[test]
fn stray_separator_is_rejected() {
    // `--` must not smuggle a subcommand past its own level
    let err = tool().try_get_matches_from(["tool", "--", "run"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    assert!(err.use_stderr());
    assert_ne!(err.exit_code(), 0);
    assert!(err.to_string().contains("run"), "{err}");

    let err = tool()
        .try_get_matches_from(["tool", "run", "--output", "o", "--", "extra"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::UnknownArgument);
    let rendered = err.to_string();
    assert!(rendered.contains("extra"), "{rendered}");
    assert!(rendered.contains("Usage: tool run"), "{rendered}");
}

#[test]
fn help_succeeds_without_required_arguments() {
    let err = tool().try_get_matches_from(["tool", "--help"]).unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    assert!(!err.use_stderr());
    assert_eq!(err.exit_code(), 0);
    let rendered = err.to_string();
    assert!(!rendered.contains("error:"), "{rendered}");
    assert!(rendered.contains("run"), "{rendered}");
    assert!(rendered.contains("config"), "{rendered}");
    assert!(rendered.contains("Usage: tool"), "{rendered}");

    // `--output` is required, yet asking for help must still succeed
    let err = tool()
        .try_get_matches_from(["tool", "run", "--help"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    assert!(!err.use_stderr());
    assert_eq!(err.exit_code(), 0);
    let rendered = err.to_string();
    assert!(!rendered.contains("error:"), "{rendered}");
    assert!(rendered.contains("Usage: tool run"), "{rendered}");
    assert!(rendered.contains("--output <PATH>"), "{rendered}");
    assert!(rendered.contains("--json"), "{rendered}");
    assert!(rendered.contains("--yaml"), "{rendered}");

    let err = tool()
        .try_get_matches_from(["tool", "config", "--help"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::DisplayHelp);
    assert!(!err.use_stderr());
    assert_eq!(err.exit_code(), 0);
    let rendered = err.to_string();
    assert!(!rendered.contains("error:"), "{rendered}");
    assert!(rendered.contains("set"), "{rendered}");
    assert!(rendered.contains("show"), "{rendered}");
}

#[test]
fn failed_parse_does_not_pollute_the_command_or_later_parses() {
    let cmd = tool();

    // A failing parse must not leave partial state behind in the definition
    let err = cmd
        .clone()
        .try_get_matches_from(["tool", "run", "--json", "--yaml"])
        .unwrap_err();
    assert_eq!(err.kind(), ErrorKind::ArgumentConflict);

    let matches = cmd
        .clone()
        .try_get_matches_from(["tool", "run", "--output", "out.txt", "--yaml"])
        .unwrap();
    let run = run_matches(&matches);
    assert_eq!(
        run.get_one::<String>("output").map(String::as_str),
        Some("out.txt")
    );
    assert!(run.get_flag("yaml"));

    // Sibling subcommands are unaffected by the earlier failure
    let matches = cmd
        .clone()
        .try_get_matches_from(["tool", "config", "show"])
        .unwrap();
    assert!(matches!(matches.subcommand(), Some(("config", _))));

    // Help output is byte-for-byte identical before and after a failure
    let help_before = tool()
        .try_get_matches_from(["tool", "run", "--help"])
        .unwrap_err()
        .to_string();
    let help_after = cmd
        .clone()
        .try_get_matches_from(["tool", "run", "--help"])
        .unwrap_err()
        .to_string();
    assert_eq!(help_before, help_after);
}

#[test]
fn errors_and_usage_are_stable_across_identical_parses() {
    let first = tool()
        .try_get_matches_from(["tool", "run"])
        .unwrap_err()
        .to_string();
    let second = tool()
        .try_get_matches_from(["tool", "run"])
        .unwrap_err()
        .to_string();
    assert_eq!(first, second);

    let first = tool()
        .try_get_matches_from(["tool", "run", "--output", "o", "--json", "--yaml"])
        .unwrap_err()
        .to_string();
    let second = tool()
        .try_get_matches_from(["tool", "run", "--output", "o", "--json", "--yaml"])
        .unwrap_err()
        .to_string();
    assert_eq!(first, second);
}

#[test]
fn value_order_follows_definition_not_token_order() {
    // Reordering the flags must not reorder or alter the matched values
    let a = tool()
        .try_get_matches_from(["tool", "run", "--yaml", "--output", "first"])
        .unwrap();
    let b = tool()
        .try_get_matches_from(["tool", "run", "--output", "first", "--yaml"])
        .unwrap();
    let (a, b) = (run_matches(&a), run_matches(&b));
    assert_eq!(
        a.get_one::<String>("output"),
        b.get_one::<String>("output")
    );
    assert_eq!(a.get_flag("yaml"), b.get_flag("yaml"));

    // Positionals keep their defined roles
    let matches = tool()
        .try_get_matches_from(["tool", "config", "set", "color", "always"])
        .unwrap();
    let Some(("config", config)) = matches.subcommand() else {
        panic!("expected `config` subcommand")
    };
    let Some(("set", set)) = config.subcommand() else {
        panic!("expected `set` subcommand")
    };
    assert_eq!(
        set.get_one::<String>("key").map(String::as_str),
        Some("color")
    );
    assert_eq!(
        set.get_one::<String>("value").map(String::as_str),
        Some("always")
    );
}
