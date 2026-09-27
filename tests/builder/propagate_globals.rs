use clap::{Arg, ArgAction, ArgMatches, Command};

fn get_app() -> Command {
    Command::new("myprog")
        .arg(
            Arg::new("GLOBAL_ARG")
                .long("global-arg")
                .help("Specifies something needed by the subcommands")
                .global(true)
                .action(ArgAction::Set)
                .default_value("default_value"),
        )
        .arg(
            Arg::new("GLOBAL_FLAG")
                .long("global-flag")
                .help("Specifies something needed by the subcommands")
                .global(true)
                .action(ArgAction::Count),
        )
        .subcommand(Command::new("outer").defer(|cmd| cmd.subcommand(Command::new("inner"))))
}

fn get_matches(cmd: Command, argv: &'static str) -> ArgMatches {
    cmd.try_get_matches_from(argv.split(' ').collect::<Vec<_>>())
        .unwrap()
}

fn get_outer_matches(m: &ArgMatches) -> &ArgMatches {
    m.subcommand_matches("outer")
        .expect("could not access outer subcommand")
}

fn get_inner_matches(m: &ArgMatches) -> &ArgMatches {
    get_outer_matches(m)
        .subcommand_matches("inner")
        .expect("could not access inner subcommand")
}

fn top_can_access_arg<T: Into<Option<&'static str>>>(m: &ArgMatches, val: T) -> bool {
    m.get_one::<String>("GLOBAL_ARG").map(|v| v.as_str()) == val.into()
}

fn inner_can_access_arg<T: Into<Option<&'static str>>>(m: &ArgMatches, val: T) -> bool {
    get_inner_matches(m)
        .get_one::<String>("GLOBAL_ARG")
        .map(|v| v.as_str())
        == val.into()
}

fn outer_can_access_arg<T: Into<Option<&'static str>>>(m: &ArgMatches, val: T) -> bool {
    get_outer_matches(m)
        .get_one::<String>("GLOBAL_ARG")
        .map(|v| v.as_str())
        == val.into()
}

fn top_can_access_flag(m: &ArgMatches, present: bool, occurrences: u8) -> bool {
    (m.contains_id("GLOBAL_FLAG") == present)
        && (m.get_one::<u8>("GLOBAL_FLAG").copied() == Some(occurrences))
}

fn inner_can_access_flag(m: &ArgMatches, present: bool, occurrences: u8) -> bool {
    let m = get_inner_matches(m);
    (m.contains_id("GLOBAL_FLAG") == present)
        && (m.get_one::<u8>("GLOBAL_FLAG").copied() == Some(occurrences))
}

fn outer_can_access_flag(m: &ArgMatches, present: bool, occurrences: u8) -> bool {
    let m = get_outer_matches(m);
    (m.contains_id("GLOBAL_FLAG") == present)
        && (m.get_one::<u8>("GLOBAL_FLAG").copied() == Some(occurrences))
}

#[test]
fn global_arg_used_top_level() {
    let m = get_matches(get_app(), "myprog --global-arg=some_value outer inner");

    assert!(top_can_access_arg(&m, "some_value"));
    assert!(inner_can_access_arg(&m, "some_value"));
    assert!(outer_can_access_arg(&m, "some_value"));
}

#[test]
fn global_arg_used_outer() {
    let m = get_matches(get_app(), "myprog outer --global-arg=some_value inner");

    assert!(top_can_access_arg(&m, "some_value"));
    assert!(inner_can_access_arg(&m, "some_value"));
    assert!(outer_can_access_arg(&m, "some_value"));
}

#[test]
fn global_arg_used_inner() {
    let m = get_matches(get_app(), "myprog outer inner --global-arg=some_value");

    assert!(top_can_access_arg(&m, "some_value"));
    assert!(inner_can_access_arg(&m, "some_value"));
    assert!(outer_can_access_arg(&m, "some_value"));
}

#[test]
fn global_arg_default_value() {
    let m = get_matches(get_app(), "myprog outer inner");

    assert!(top_can_access_arg(&m, "default_value"));
    assert!(inner_can_access_arg(&m, "default_value"));
    assert!(outer_can_access_arg(&m, "default_value"));
}

#[test]
fn global_flag_used_top_level() {
    let m = get_matches(get_app(), "myprog --global-flag outer inner");

    assert!(top_can_access_flag(&m, true, 1));
    assert!(inner_can_access_flag(&m, true, 1));
    assert!(outer_can_access_flag(&m, true, 1));
}

#[test]
fn global_flag_used_outer() {
    let m = get_matches(get_app(), "myprog outer --global-flag inner");

    assert!(top_can_access_flag(&m, true, 1));
    assert!(inner_can_access_flag(&m, true, 1));
    assert!(outer_can_access_flag(&m, true, 1));
}

#[test]
fn global_flag_used_inner() {
    let m = get_matches(get_app(), "myprog outer inner --global-flag");

    assert!(top_can_access_flag(&m, true, 1));
    assert!(inner_can_access_flag(&m, true, 1));
    assert!(outer_can_access_flag(&m, true, 1));
}

#[test]
fn global_flag_2x_used_top_level() {
    let m = get_matches(get_app(), "myprog --global-flag --global-flag outer inner");

    assert!(top_can_access_flag(&m, true, 2));
    assert!(inner_can_access_flag(&m, true, 2));
    assert!(outer_can_access_flag(&m, true, 2));
}

#[test]
fn global_flag_2x_used_inner() {
    let m = get_matches(get_app(), "myprog outer inner --global-flag --global-flag");

    assert!(top_can_access_flag(&m, true, 2));
    assert!(inner_can_access_flag(&m, true, 2));
    assert!(outer_can_access_flag(&m, true, 2));
}

fn get_append_app() -> Command {
    Command::new("myprog")
        .arg(
            Arg::new("GLOBAL_ARG")
                .long("global-arg")
                .help("Specifies something needed by the subcommands")
                .global(true)
                .action(ArgAction::Append),
        )
        .subcommand(Command::new("outer").defer(|cmd| cmd.subcommand(Command::new("inner"))))
}

fn get_values(m: &ArgMatches) -> Vec<String> {
    m.get_many::<String>("GLOBAL_ARG")
        .unwrap_or_default()
        .cloned()
        .collect()
}

#[test]
fn global_append_arg_used_top_level() {
    let m = get_append_app()
        .try_get_matches_from(["myprog", "--global-arg", "a", "--global-arg", "b", "outer", "inner"])
        .unwrap();

    let expected = vec!["a".to_owned(), "b".to_owned()];
    assert_eq!(get_values(&m), expected);
    assert_eq!(get_values(get_outer_matches(&m)), expected);
    assert_eq!(get_values(get_inner_matches(&m)), expected);
}

#[test]
fn global_append_arg_used_inner() {
    let m = get_append_app()
        .try_get_matches_from(["myprog", "outer", "inner", "--global-arg", "a", "--global-arg", "b"])
        .unwrap();

    let expected = vec!["a".to_owned(), "b".to_owned()];
    assert_eq!(get_values(&m), expected);
    assert_eq!(get_values(get_outer_matches(&m)), expected);
    assert_eq!(get_values(get_inner_matches(&m)), expected);
}

#[test]
fn global_append_arg_used_at_every_level() {
    let m = get_append_app()
        .try_get_matches_from([
            "myprog",
            "--global-arg",
            "a",
            "outer",
            "--global-arg",
            "b",
            "inner",
            "--global-arg",
            "c",
        ])
        .unwrap();

    // Values accumulate in command-line order, regardless of the level they appeared at
    let expected = vec!["a".to_owned(), "b".to_owned(), "c".to_owned()];
    assert_eq!(get_values(&m), expected);
    assert_eq!(get_values(get_outer_matches(&m)), expected);
    assert_eq!(get_values(get_inner_matches(&m)), expected);
}
