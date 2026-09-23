use clap::arg;
use snapbox::assert_data_eq;
use snapbox::prelude::*;
use snapbox::str;

#[test]
fn arg_long() {
    let arg = arg!(--long);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"long"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
Some(
    "long",
)

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
SetTrue

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
false

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
#[should_panic = "Without a value or long flag, the `name:` prefix is required"]
fn arg_short() {
    arg!(-s);
}

#[test]
fn arg_long_dashed() {
    let arg = arg!(--"long-flag");
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"long-flag"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
Some(
    "long-flag",
)

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
SetTrue

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
false

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_long_optional_value() {
    let arg = arg!(--long[VALUE]);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"long"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
Some(
    "long",
)

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
false

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE",
    ],
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_long_required_value() {
    let arg = arg!(--long <VALUE>);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"long"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
Some(
    "long",
)

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
false

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE",
    ],
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_long_multiple_values() {
    let arg = arg!(--long <VALUE1> <VALUE2> <VALUE3> [VALUE4] [VALUE5]);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"long"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
Some(
    "long",
)

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
false

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE1",
        "VALUE2",
        "VALUE3",
        "VALUE4",
        "VALUE5",
    ],
)

"#]]);
    assert_data_eq!(arg.get_num_args().to_debug(), str![[r#"
Some(
    3..=5,
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_long_multiple_values_parse() {
    let arg = arg!(--copy <SRC> <DST> [MODE]);
    assert_eq!(
        arg.get_value_names(),
        Some(vec!["SRC".into(), "DST".into(), "MODE".into()].as_slice())
    );
    assert_eq!(arg.get_num_args(), Some((2..=3).into()));

    let mut cmd = clap::Command::new("tool").arg(arg);

    let m = cmd
        .try_get_matches_from_mut(["tool", "--copy", "a", "b"])
        .unwrap();
    let values: Vec<_> = m
        .get_many::<String>("copy")
        .unwrap()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(values, ["a", "b"]);

    let m = cmd
        .try_get_matches_from_mut(["tool", "--copy", "a", "b", "fast"])
        .unwrap();
    let values: Vec<_> = m
        .get_many::<String>("copy")
        .unwrap()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(values, ["a", "b", "fast"]);

    let err = cmd
        .try_get_matches_from_mut(["tool", "--copy", "a"])
        .unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::TooFewValues);

    let help = cmd.render_help().to_string();
    assert!(
        help.contains("--copy <SRC> <DST> [MODE]"),
        "help was:\n{help}"
    );
}

#[test]
fn arg_long_multiple_value_names_string_literals() {
    let arg = arg!(--copy <"src"> <"dst"> ["mode"]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "src",
        "dst",
        "mode",
    ],
)

"#]]);
    assert_data_eq!(arg.get_num_args().to_debug(), str![[r#"
Some(
    2..=3,
)

"#]]);
}

#[test]
#[should_panic = "Required value names must precede optional value names"]
fn arg_long_optional_value_before_required() {
    arg!(--copy [MODE] <SRC>);
}

#[test]
fn arg_optional_value() {
    let arg = arg!([VALUE]);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"VALUE"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
false

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE",
    ],
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_required_value() {
    let arg = arg!(<VALUE>);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"VALUE"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
true

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE",
    ],
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_multiple_values() {
    let arg = arg!(<VALUE1> <VALUE2> <VALUE3> [VALUE4] [VALUE5]);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"VALUE1"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
true

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE1",
        "VALUE2",
        "VALUE3",
        "VALUE4",
        "VALUE5",
    ],
)

"#]]);
    assert_data_eq!(arg.get_num_args().to_debug(), str![[r#"
Some(
    3..=5,
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_named_positional_multiple_values_parse() {
    let arg = arg!(files: <IN> [OUT]);
    assert_eq!(
        arg.get_value_names(),
        Some(vec!["IN".into(), "OUT".into()].as_slice())
    );
    assert_eq!(arg.get_num_args(), Some((1..=2).into()));
    assert!(arg.is_required_set());

    let mut cmd = clap::Command::new("tool").arg(arg);

    let m = cmd.try_get_matches_from_mut(["tool", "a"]).unwrap();
    let values: Vec<_> = m
        .get_many::<String>("files")
        .unwrap()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(values, ["a"]);

    let m = cmd.try_get_matches_from_mut(["tool", "a", "b"]).unwrap();
    let values: Vec<_> = m
        .get_many::<String>("files")
        .unwrap()
        .map(|s| s.as_str())
        .collect();
    assert_eq!(values, ["a", "b"]);

    let err = cmd.try_get_matches_from_mut(["tool"]).unwrap_err();
    assert_eq!(err.kind(), clap::error::ErrorKind::MissingRequiredArgument);

    let help = cmd.render_help().to_string();
    assert!(help.contains("<IN> [OUT]"), "help was:\n{help}");
}

#[test]
#[should_panic = "Required value names must precede optional value names"]
fn arg_optional_value_before_required() {
    arg!([OUT] <IN>);
}

#[test]
fn arg_named_positional() {
    let arg = arg!(name: <VALUE>);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"name"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
true

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE",
    ],
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_named_long() {
    let arg = arg!(name: --long <VALUE>);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"name"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
Some(
    "long",
)

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
false

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE",
    ],
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}

#[test]
fn arg_named_dashed() {
    let arg = arg!("named-arg": --long <VALUE>);
    assert_data_eq!(arg.get_id().to_debug(), str![[r#"
"named-arg"

"#]]);
    assert_data_eq!(arg.get_short().to_debug(), str![[r#"
None

"#]]);
    assert_data_eq!(arg.get_long().to_debug(), str![[r#"
Some(
    "long",
)

"#]]);
    assert_data_eq!(arg.get_action().to_debug(), str![[r#"
Set

"#]]);
    assert_data_eq!(arg.is_required_set().to_debug(), str![[r#"
false

"#]]);
    assert_data_eq!(arg.get_value_names().to_debug(), str![[r#"
Some(
    [
        "VALUE",
    ],
)

"#]]);
    assert_data_eq!(arg.get_help().to_debug(), str![[r#"
None

"#]]);
}
