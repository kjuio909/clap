#![cfg(feature = "unstable-dynamic")]

use std::fs;
use std::path::Path;

use clap::{builder::PossibleValue, Command};
use clap_complete::engine::{
    ArgValueCandidates, ArgValueCompleter, CompletionCandidate, PathCompleter, SubcommandCandidates,
};
use snapbox::assert_data_eq;

macro_rules! complete {
    ($cmd:expr, $input:expr$(, current_dir = $current_dir:expr)? $(,)?) => {
        {
            #[allow(unused)]
            let current_dir: Option<&Path> = None;
            $(let current_dir = $current_dir;)?
            complete(&mut $cmd, $input, current_dir)
        }
    }
}

#[test]
fn suggest_subcommand_subset() {
    let mut cmd = Command::new("exhaustive")
        .subcommand(Command::new("hello-world"))
        .subcommand(Command::new("hello-moon"))
        .subcommand(Command::new("goodbye-world"));

    assert_data_eq!(
        complete!(cmd, "he"),
        snapbox::str![[r#"
hello-world
hello-moon
help	Print this message or the help of the given subcommand(s)
"#]],
    );
}

#[test]
fn suggest_hidden_long_flags() {
    let mut cmd = Command::new("exhaustive")
        .arg(clap::Arg::new("hello-world-visible").long("hello-world-visible"))
        .arg(
            clap::Arg::new("hello-world-hidden")
                .long("hello-world-hidden")
                .hide(true),
        );

    assert_data_eq!(
        complete!(cmd, "--hello-world"),
        snapbox::str!["--hello-world-visible"]
    );

    assert_data_eq!(
        complete!(cmd, "--hello-world-h"),
        snapbox::str!["--hello-world-hidden"]
    );
}

#[test]
fn suggest_hidden_subcommand_and_aliases() {
    let mut cmd = Command::new("exhaustive")
        .subcommand(
            Command::new("test_visible")
                .visible_alias("test_visible-alias_visible")
                .alias("test_visible-alias_hidden"),
        )
        .subcommand(
            Command::new("test_hidden")
                .visible_alias("test_hidden-alias_visible")
                .alias("test_hidden-alias_hidden")
                .hide(true),
        );

    assert_data_eq!(complete!(cmd, "test"), snapbox::str!["test_visible"]);

    assert_data_eq!(complete!(cmd, "test_h"), snapbox::str!["test_hidden"]);

    assert_data_eq!(
        complete!(cmd, "test_hidden-alias_h"),
        snapbox::str!["test_hidden-alias_hidden"]
    );
}

#[test]
fn suggest_subcommand_aliases() {
    let mut cmd = Command::new("exhaustive")
        .subcommand(
            Command::new("hello-world")
                .visible_alias("hello-world-foo")
                .alias("hidden-world"),
        )
        .subcommand(
            Command::new("hello-moon")
                .visible_alias("hello-moon-foo")
                .alias("hidden-moon"),
        )
        .subcommand(
            Command::new("goodbye-world")
                .visible_alias("goodbye-world-foo")
                .alias("hidden-goodbye"),
        );

    assert_data_eq!(
        complete!(cmd, "hello"),
        snapbox::str![[r#"
hello-world
hello-moon
"#]],
    );
}

#[test]
fn suggest_hidden_possible_value() {
    let mut cmd = Command::new("exhaustive").arg(
        clap::Arg::new("possible_value").long("test").value_parser([
            PossibleValue::new("test-visible").help("Say hello to the world"),
            PossibleValue::new("test-hidden")
                .help("Say hello to the moon")
                .hide(true),
        ]),
    );

    assert_data_eq!(
        complete!(cmd, "--test=test"),
        snapbox::str!["--test=test-visible	Say hello to the world"]
    );

    assert_data_eq!(
        complete!(cmd, "--test=test-h"),
        snapbox::str!["--test=test-hidden	Say hello to the moon"]
    );
}

#[test]
fn suggest_hidden_long_flag_aliases() {
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("test_visible")
                .long("test_visible")
                .visible_alias("test_visible-alias_visible")
                .alias("test_visible-alias_hidden"),
        )
        .arg(
            clap::Arg::new("test_hidden")
                .long("test_hidden")
                .visible_alias("test_hidden-alias_visible")
                .alias("test_hidden-alias_hidden")
                .hide(true),
        );

    assert_data_eq!(complete!(cmd, "--test"), snapbox::str!["--test_visible"]);

    assert_data_eq!(complete!(cmd, "--test_h"), snapbox::str!["--test_hidden"]);

    assert_data_eq!(
        complete!(cmd, "--test_visible-alias_h"),
        snapbox::str!["--test_visible-alias_hidden"]
    );

    assert_data_eq!(
        complete!(cmd, "--test_hidden-alias_h"),
        snapbox::str!["--test_hidden-alias_hidden"]
    );
}

#[test]
fn suggest_long_flag_subset() {
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("hello-world")
                .long("hello-world")
                .action(clap::ArgAction::Count),
        )
        .arg(
            clap::Arg::new("hello-moon")
                .long("hello-moon")
                .action(clap::ArgAction::Count),
        )
        .arg(
            clap::Arg::new("goodbye-world")
                .long("goodbye-world")
                .action(clap::ArgAction::Count),
        );

    assert_data_eq!(
        complete!(cmd, "--he"),
        snapbox::str![[r#"
--hello-world
--hello-moon
--help	Print help
"#]],
    );
}

#[test]
fn suggest_possible_value_subset() {
    let name = "exhaustive";
    let mut cmd = Command::new(name).arg(clap::Arg::new("hello-world").value_parser([
        PossibleValue::new("hello-world").help("Say hello to the world"),
        "hello-moon".into(),
        "goodbye-world".into(),
    ]));

    assert_data_eq!(
        complete!(cmd, "hello"),
        snapbox::str![[r#"
hello-world	Say hello to the world
hello-moon
"#]],
    );
}

#[test]
fn suggest_additional_short_flags() {
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("a")
                .short('a')
                .action(clap::ArgAction::Count),
        )
        .arg(
            clap::Arg::new("b")
                .short('b')
                .action(clap::ArgAction::Count),
        )
        .arg(
            clap::Arg::new("c")
                .short('c')
                .action(clap::ArgAction::Count),
        );

    assert_data_eq!(
        complete!(cmd, "-a"),
        snapbox::str![[r#"
-aa
-ab
-ac
-ah	Print help
"#]],
    );
}

#[test]
fn suggest_subcommand_positional() {
    let mut cmd = Command::new("exhaustive").subcommand(Command::new("hello-world").arg(
        clap::Arg::new("hello-world").value_parser([
            PossibleValue::new("hello-world").help("Say hello to the world"),
            "hello-moon".into(),
            "goodbye-world".into(),
        ]),
    ));

    assert_data_eq!(
        complete!(cmd, "hello-world [TAB]"),
        snapbox::str![[r#"
hello-world	Say hello to the world
hello-moon
goodbye-world
--help	Print help (see more with '--help')
"#]],
    );
}

#[test]
fn suggest_subcommand_positional_after_escape() {
    let mut cmd = Command::new("exhaustive").subcommand(Command::new("hello-world").arg(
        clap::Arg::new("hello-world").value_parser([
            PossibleValue::new("hello-world").help("Say hello to the world"),
            "hello-moon".into(),
            "goodbye-world".into(),
        ]),
    ));

    assert_data_eq!(
        complete!(cmd, "hello-world -- [TAB]"),
        snapbox::str![[r#"
hello-world	Say hello to the world
hello-moon
goodbye-world
"#]],
    );
}

#[test]
fn suggest_multiple_positional_after_escape() {
    let mut cmd =
        Command::new("exhaustive").arg(clap::Arg::new("hello-world").num_args(0..).value_parser([
            PossibleValue::new("hello-world"),
            "hello-moon".into(),
            "goodbye-world".into(),
        ]));

    assert_data_eq!(
        complete!(cmd, "-- hello-moon [TAB]"),
        snapbox::str![[r#"
hello-world
hello-moon
goodbye-world
"#]],
    );
}

#[test]
fn suggest_argument_value() {
    let mut cmd = Command::new("dynamic")
        .arg(
            clap::Arg::new("format")
                .long("format")
                .short('F')
                .value_parser(["json", "yaml", "toml"]),
        )
        .arg(
            clap::Arg::new("stream")
                .long("stream")
                .short('S')
                .value_parser(["stdout", "stderr"]),
        )
        .arg(
            clap::Arg::new("count")
                .long("count")
                .short('c')
                .action(clap::ArgAction::Count),
        )
        .arg(clap::Arg::new("positional").value_parser(["pos_a", "pos_b", "pos_c"]))
        .args_conflicts_with_subcommands(true);

    assert_data_eq!(
        complete!(cmd, "--format [TAB]"),
        snapbox::str![[r#"
json
yaml
toml
"#]],
    );

    assert_data_eq!(
        complete!(cmd, "-F [TAB]"),
        snapbox::str![[r#"
json
yaml
toml
"#]],
    );

    assert_data_eq!(complete!(cmd, "--format j[TAB]"), snapbox::str!["json"],);

    assert_data_eq!(complete!(cmd, "-F j[TAB]"), snapbox::str!["json"],);

    assert_data_eq!(complete!(cmd, "--format t[TAB]"), snapbox::str!["toml"],);

    assert_data_eq!(complete!(cmd, "-F t[TAB]"), snapbox::str!["toml"],);

    assert_data_eq!(
        complete!(cmd, "-cccF [TAB]"),
        snapbox::str![[r#"
json
yaml
toml
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format toml [TAB]"),
        snapbox::str![[r#"
pos_a
pos_b
pos_c
--format
--stream
--count
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-cS[TAB]"),
        snapbox::str![[r#"
-cSstdout
-cSstderr
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-cS=[TAB]"),
        snapbox::str![[r#"
-cS=stdout
-cS=stderr
"#]]
    );

    assert_data_eq!(complete!(cmd, "-cS=stdo[TAB]"), snapbox::str!["-cS=stdout"]);

    assert_data_eq!(complete!(cmd, "-cSF[TAB]"), snapbox::str![]);

    assert_data_eq!(complete!(cmd, "-cSF=[TAB]"), snapbox::str![]);
}

#[test]
fn suggest_argument_multi_values() {
    let mut cmd = Command::new("dynamic")
        .arg(
            clap::Arg::new("certain-num")
                .long("certain-num")
                .short('Y')
                .value_parser(["val1", "val2", "val3"])
                .num_args(3),
        )
        .arg(
            clap::Arg::new("uncertain-num")
                .long("uncertain-num")
                .short('N')
                .value_parser(["val1", "val2", "val3"])
                .num_args(1..=3),
        );

    assert_data_eq!(
        complete!(cmd, "--certain-num [TAB]"),
        snapbox::str![[r#"
val1
val2
val3
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--certain-num val1 [TAB]"),
        snapbox::str![[r#"
val1
val2
val3
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--certain-num val1 val2 val3 [TAB]"),
        snapbox::str![[r#"
--certain-num
--uncertain-num
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--uncertain-num [TAB]"),
        snapbox::str![[r#"
val1
val2
val3
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--uncertain-num val1 [TAB]"),
        snapbox::str![[r#"
val1
val2
val3
--certain-num
--uncertain-num
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--uncertain-num val1 val2 val3 [TAB]"),
        snapbox::str![[r#"
--certain-num
--uncertain-num
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-Y [TAB]"),
        snapbox::str![[r#"
val1
val2
val3
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-Y val1 [TAB]"),
        snapbox::str![[r#"
val1
val2
val3
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-Y val1 val2 val3 [TAB]"),
        snapbox::str![[r#"
--certain-num
--uncertain-num
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-N [TAB]"),
        snapbox::str![[r#"
val1
val2
val3
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-N val1 [TAB]"),
        snapbox::str![[r#"
val1
val2
val3
--certain-num
--uncertain-num
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-N val1 val2 val3 [TAB]"),
        snapbox::str![[r#"
--certain-num
--uncertain-num
--help	Print help
"#]]
    );
}

#[test]
fn suggest_value_hint_file_path() {
    let mut cmd = Command::new("dynamic")
        .arg(
            clap::Arg::new("input")
                .long("input")
                .short('i')
                .value_hint(clap::ValueHint::FilePath),
        )
        .args_conflicts_with_subcommands(true);

    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let testdir_path = testdir.path().unwrap();

    fs::write(testdir_path.join("a_file"), "").unwrap();
    fs::write(testdir_path.join(".a_file"), "").unwrap();
    fs::write(testdir_path.join("b_file"), "").unwrap();
    fs::write(testdir_path.join(".b_file"), "").unwrap();
    fs::create_dir_all(testdir_path.join("c_dir")).unwrap();
    fs::create_dir_all(testdir_path.join(".c_dir")).unwrap();
    fs::create_dir_all(testdir_path.join("d_dir")).unwrap();
    fs::create_dir_all(testdir_path.join(".d_dir")).unwrap();

    assert_data_eq!(
        complete!(cmd, "--input [TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
a_file
b_file
c_dir/
d_dir/
"#]],
    );

    assert_data_eq!(
        complete!(cmd, "--input a[TAB]", current_dir = Some(testdir_path)),
        snapbox::str!["a_file"],
    );
    assert_data_eq!(
        complete!(cmd, "--input .[TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
./a_file
./b_file
./c_dir/
./d_dir/
"#]],
    );
    assert_data_eq!(
        complete!(cmd, "--input .a[TAB]", current_dir = Some(testdir_path)),
        snapbox::str![".a_file"],
    );
}

#[test]
fn suggest_value_path_file() {
    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let testdir_path = testdir.path().unwrap();

    fs::write(testdir_path.join("a_file"), "").unwrap();
    fs::write(testdir_path.join(".a_file"), "").unwrap();
    fs::write(testdir_path.join("b_file"), "").unwrap();
    fs::write(testdir_path.join(".b_file"), "").unwrap();
    fs::create_dir_all(testdir_path.join("c_dir")).unwrap();
    fs::create_dir_all(testdir_path.join(".c_dir")).unwrap();
    fs::create_dir_all(testdir_path.join("d_dir")).unwrap();
    fs::create_dir_all(testdir_path.join(".d_dir")).unwrap();

    let mut cmd = Command::new("dynamic")
        .arg(
            clap::Arg::new("input")
                .long("input")
                .short('i')
                .add(ArgValueCompleter::new(
                    PathCompleter::file()
                        .stdio()
                        .current_dir(testdir_path.to_owned()),
                )),
        )
        .args_conflicts_with_subcommands(true);

    assert_data_eq!(
        complete!(cmd, "--input [TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
a_file
b_file
c_dir/
d_dir/
-	stdio
"#]],
    );

    assert_data_eq!(
        complete!(cmd, "--input a[TAB]", current_dir = Some(testdir_path)),
        snapbox::str!["a_file"],
    );
    assert_data_eq!(
        complete!(cmd, "--input .[TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
./a_file
./b_file
./c_dir/
./d_dir/
"#]],
    );
    assert_data_eq!(
        complete!(cmd, "--input .a[TAB]", current_dir = Some(testdir_path)),
        snapbox::str![".a_file"],
    );
}

#[test]
fn suggest_value_path_dir() {
    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let testdir_path = testdir.path().unwrap();

    fs::write(testdir_path.join("a_file"), "").unwrap();
    fs::write(testdir_path.join(".a_file"), "").unwrap();
    fs::write(testdir_path.join("b_file"), "").unwrap();
    fs::write(testdir_path.join(".b_file"), "").unwrap();
    fs::create_dir_all(testdir_path.join("c_dir")).unwrap();
    fs::create_dir_all(testdir_path.join(".c_dir")).unwrap();
    fs::create_dir_all(testdir_path.join("d_dir")).unwrap();
    fs::create_dir_all(testdir_path.join(".d_dir")).unwrap();

    let mut cmd = Command::new("dynamic")
        .arg(
            clap::Arg::new("input")
                .long("input")
                .short('i')
                .add(ArgValueCompleter::new(
                    PathCompleter::dir().current_dir(testdir_path.to_owned()),
                )),
        )
        .args_conflicts_with_subcommands(true);

    assert_data_eq!(
        complete!(cmd, "--input [TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
.
c_dir/
d_dir/
"#]],
    );

    assert_data_eq!(
        complete!(cmd, "--input c[TAB]", current_dir = Some(testdir_path)),
        snapbox::str!["c_dir/"],
    );
    assert_data_eq!(
        complete!(cmd, "--input .[TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
./c_dir/
./d_dir/
"#]],
    );
    assert_data_eq!(
        complete!(cmd, "--input .c[TAB]", current_dir = Some(testdir_path)),
        snapbox::str![".c_dir/"],
    );
}

#[cfg(unix)]
#[test]
fn suggest_value_hint_file_path_symlink_to_dir() {
    use std::os::unix::fs::symlink;

    let mut cmd = Command::new("dynamic").arg(
        clap::Arg::new("input")
            .long("input")
            .short('i')
            .value_hint(clap::ValueHint::FilePath),
    );

    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let testdir_path = testdir.path().unwrap();

    fs::create_dir_all(testdir_path.join("real_dir")).unwrap();
    fs::write(testdir_path.join("real_dir/file.txt"), "").unwrap();
    symlink("real_dir", testdir_path.join("link_dir")).unwrap();

    // Symlink to directory should appear with trailing slash
    assert_data_eq!(
        complete!(cmd, "--input [TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
link_dir/
real_dir/
"#]],
    );

    // Should be able to complete through the symlink
    assert_data_eq!(
        complete!(cmd, "--input link_dir/[TAB]", current_dir = Some(testdir_path)),
        snapbox::str!["link_dir/file.txt"],
    );
}

#[cfg(unix)]
#[test]
fn suggest_value_hint_file_path_symlink_to_file() {
    use std::os::unix::fs::symlink;

    let mut cmd = Command::new("dynamic").arg(
        clap::Arg::new("input")
            .long("input")
            .short('i')
            .value_hint(clap::ValueHint::FilePath),
    );

    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let testdir_path = testdir.path().unwrap();

    fs::write(testdir_path.join("real_file.txt"), "").unwrap();
    symlink("real_file.txt", testdir_path.join("link_file.txt")).unwrap();

    // Symlink to file should appear without trailing slash
    assert_data_eq!(
        complete!(cmd, "--input [TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
link_file.txt
real_file.txt
"#]],
    );
}

#[cfg(unix)]
#[test]
fn suggest_value_hint_dir_path_symlink() {
    use std::os::unix::fs::symlink;

    let mut cmd = Command::new("dynamic").arg(
        clap::Arg::new("input")
            .long("input")
            .short('i')
            .value_hint(clap::ValueHint::DirPath),
    );

    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let testdir_path = testdir.path().unwrap();

    fs::create_dir_all(testdir_path.join("real_dir")).unwrap();
    fs::write(testdir_path.join("real_file.txt"), "").unwrap();
    symlink("real_dir", testdir_path.join("link_dir")).unwrap();
    symlink("real_file.txt", testdir_path.join("link_file.txt")).unwrap();

    // Symlink to directory should have trailing slash
    assert_data_eq!(
        complete!(cmd, "--input [TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
.
link_dir/
real_dir/
"#]],
    );
}

#[cfg(unix)]
#[test]
fn suggest_value_hint_file_path_broken_symlink() {
    use std::os::unix::fs::symlink;

    let mut cmd = Command::new("dynamic").arg(
        clap::Arg::new("input")
            .long("input")
            .short('i')
            .value_hint(clap::ValueHint::FilePath),
    );

    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let testdir_path = testdir.path().unwrap();

    fs::write(testdir_path.join("real_file.txt"), "").unwrap();
    symlink("nonexistent", testdir_path.join("broken_link")).unwrap();

    // Broken symlink should not appear for FilePath (target doesn't exist)
    // but should not cause a crash
    assert_data_eq!(
        complete!(cmd, "--input [TAB]", current_dir = Some(testdir_path)),
        snapbox::str!["real_file.txt"],
    );
}

#[cfg(unix)]
#[test]
fn suggest_value_hint_any_path_broken_symlink() {
    use std::os::unix::fs::symlink;

    let mut cmd = Command::new("dynamic").arg(
        clap::Arg::new("input")
            .long("input")
            .short('i')
            .value_hint(clap::ValueHint::AnyPath),
    );

    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let testdir_path = testdir.path().unwrap();

    fs::write(testdir_path.join("real_file.txt"), "").unwrap();
    symlink("nonexistent", testdir_path.join("broken_link")).unwrap();

    // Broken symlink should appear for AnyPath since filter is |_| true
    assert_data_eq!(
        complete!(cmd, "--input [TAB]", current_dir = Some(testdir_path)),
        snapbox::str![[r#"
.
broken_link
real_file.txt
"#]],
    );
}

#[test]
fn suggest_custom_arg_value() {
    fn custom_completer() -> Vec<CompletionCandidate> {
        vec![
            CompletionCandidate::new("foo"),
            CompletionCandidate::new("bar"),
            CompletionCandidate::new("baz"),
        ]
    }

    let mut cmd = Command::new("dynamic").arg(
        clap::Arg::new("custom")
            .long("custom")
            .add(ArgValueCandidates::new(custom_completer)),
    );

    assert_data_eq!(
        complete!(cmd, "--custom [TAB]"),
        snapbox::str![[r#"
foo
bar
baz
"#]],
    );

    assert_data_eq!(
        complete!(cmd, "--custom b[TAB]"),
        snapbox::str![[r#"
bar
baz
"#]],
    );
}

#[test]
fn suggest_custom_arg_completer() {
    fn custom_completer(current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
        let mut completions = vec![];
        let Some(current) = current.to_str() else {
            return completions;
        };

        if "foo".starts_with(current) {
            completions.push(CompletionCandidate::new("foo"));
        }
        if "bar".starts_with(current) {
            completions.push(CompletionCandidate::new("bar"));
        }
        if "baz".starts_with(current) {
            completions.push(CompletionCandidate::new("baz"));
        }
        completions
    }

    let mut cmd = Command::new("dynamic").arg(
        clap::Arg::new("custom")
            .long("custom")
            .add(ArgValueCompleter::new(custom_completer)),
    );

    assert_data_eq!(
        complete!(cmd, "--custom [TAB]"),
        snapbox::str![[r#"
foo
bar
baz
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "--custom b[TAB]"),
        snapbox::str![[r#"
bar
baz
"#]]
    );
}

#[test]
fn suggest_custom_arg_completer_at_index() {
    struct UpstreamCompleter;

    impl clap_complete::engine::ValueCompleter for UpstreamCompleter {
        fn complete(&self, _current: &std::ffi::OsStr) -> Vec<CompletionCandidate> {
            // Falls back when callers use the index-unaware path.
            vec![CompletionCandidate::new("unindexed")]
        }

        fn complete_at(
            &self,
            arg_index: usize,
            current: &std::ffi::OsStr,
        ) -> Vec<CompletionCandidate> {
            let prefix = current.to_str().unwrap_or("");
            match arg_index {
                0 => ["origin", "upstream"]
                    .into_iter()
                    .filter(|name| name.starts_with(prefix))
                    .map(CompletionCandidate::new)
                    .collect(),
                1 => ["main", "master", "dev"]
                    .into_iter()
                    .filter(|name| name.starts_with(prefix))
                    .map(CompletionCandidate::new)
                    .collect(),
                _ => Vec::new(),
            }
        }
    }

    let mut cmd = Command::new("dynamic").arg(
        clap::Arg::new("set-upstream")
            .long("set-upstream")
            .short('u')
            .num_args(2)
            .value_names(["REMOTE", "BRANCH"])
            .add(ArgValueCompleter::new(UpstreamCompleter)),
    );

    assert_data_eq!(
        complete!(cmd, "--set-upstream [TAB]"),
        snapbox::str![[r#"
origin
upstream
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "--set-upstream o[TAB]"),
        snapbox::str!["origin"]
    );
    assert_data_eq!(
        complete!(cmd, "--set-upstream origin [TAB]"),
        snapbox::str![[r#"
main
master
dev
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "--set-upstream origin m[TAB]"),
        snapbox::str![[r#"
main
master
"#]]
    );
}

#[test]
fn suggest_multi_positional() {
    let mut cmd = Command::new("dynamic")
        .arg(clap::Arg::new("positional-1").value_parser(["pos_1_a", "pos_1_b", "pos_1_c"]))
        .arg(
            clap::Arg::new("positional-2")
                .value_parser(["pos_2_a", "pos_2_b", "pos_2_c"])
                .num_args(3),
        )
        .arg(
            clap::Arg::new("--format")
                .long("format")
                .short('F')
                .value_parser(["json", "yaml", "toml"]),
        );

    assert_data_eq!(
        complete!(cmd, "pos_1_a pos_2_a [TAB]"),
        snapbox::str![[r#"
pos_2_a
pos_2_b
pos_2_c
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "pos_1_a pos_2_a pos_2_b [TAB]"),
        snapbox::str![[r#"
pos_2_a
pos_2_b
pos_2_c
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format json pos_1_a [TAB]"),
        snapbox::str![[r#"
pos_2_a
pos_2_b
pos_2_c
--format
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format json pos_1_a pos_2_a [TAB]"),
        snapbox::str![[r#"
pos_2_a
pos_2_b
pos_2_c
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format json pos_1_a pos_2_a pos_2_b pos_2_c [TAB]"),
        snapbox::str![[r#"
--format
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format json -- pos_1_a pos_2_a [TAB]"),
        snapbox::str![[r#"
pos_2_a
pos_2_b
pos_2_c
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format json -- pos_1_a pos_2_a pos_2_b [TAB]"),
        snapbox::str![[r#"
pos_2_a
pos_2_b
pos_2_c
"#]]
    );

    assert_data_eq!(
        complete!(
            cmd,
            "--format json -- pos_1_a pos_2_a pos_2_b pos_2_c [TAB]"
        ),
        snapbox::str![]
    );
}

#[test]
fn suggest_multi_positional_unbounded() {
    let mut cmd = Command::new("dynamic")
        .arg(
            clap::Arg::new("positional-1")
                .value_parser(["pos_1_a", "pos_1_b", "pos_1_c"])
                .num_args(2..),
        )
        .arg(
            clap::Arg::new("--format")
                .long("format")
                .short('F')
                .value_parser(["json", "yaml", "toml"]),
        );

    assert_data_eq!(
        complete!(cmd, "pos_1_a [TAB]"),
        snapbox::str![[r#"
pos_1_a
pos_1_b
pos_1_c
"#]]
    );
    assert_data_eq!(complete!(cmd, "pos_1_a --[TAB]"), snapbox::str![""]);
    assert_data_eq!(
        complete!(cmd, "pos_1_a --format [TAB]"),
        snapbox::str![[r#"
json
yaml
toml
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "pos_1_a --format json [TAB]"),
        snapbox::str![[r#"
pos_1_a
pos_1_b
pos_1_c
--format
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "pos_1_a pos_1_b [TAB]"),
        snapbox::str![[r#"
pos_1_a
pos_1_b
pos_1_c
--format
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "pos_1_a pos_1_b --[TAB]"),
        snapbox::str![[r#"
--format
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "pos_1_a pos_1_b --format [TAB]"),
        snapbox::str![[r#"
json
yaml
toml
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "pos_1_a pos_1_b --format json [TAB]"),
        snapbox::str![[r#"
pos_1_a
pos_1_b
pos_1_c
--format
--help	Print help
"#]]
    );
}

#[test]
fn suggest_delimiter_values() {
    let mut cmd = Command::new("delimiter")
        .arg(
            clap::Arg::new("delimiter")
                .long("delimiter")
                .short('D')
                .value_parser([
                    PossibleValue::new("comma"),
                    PossibleValue::new("space"),
                    PossibleValue::new("tab"),
                ])
                .value_delimiter(','),
        )
        .arg(
            clap::Arg::new("pos")
                .value_parser(["a_pos", "b_pos", "c_pos"])
                .value_delimiter(','),
        );

    assert_data_eq!(
        complete!(cmd, "--delimiter [TAB]"),
        snapbox::str![[r#"
comma
space
tab
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--delimiter=[TAB]"),
        snapbox::str![[r#"
--delimiter=comma
--delimiter=space
--delimiter=tab
"#]]
    );

    assert_data_eq!(complete!(cmd, "--delimiter c[TAB]"), snapbox::str!["comma"]);

    assert_data_eq!(
        complete!(cmd, "--delimiter=c[TAB]"),
        snapbox::str!["--delimiter=comma"]
    );

    assert_data_eq!(
        complete!(cmd, "--delimiter comma,[TAB]"),
        snapbox::str![[r#"
comma,comma
comma,space
comma,tab
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--delimiter=comma,[TAB]"),
        snapbox::str![[r#"
--delimiter=comma,comma
--delimiter=comma,space
--delimiter=comma,tab
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--delimiter comma,s[TAB]"),
        snapbox::str!["comma,space"]
    );

    assert_data_eq!(
        complete!(cmd, "--delimiter=comma,s[TAB]"),
        snapbox::str!["--delimiter=comma,space"]
    );

    assert_data_eq!(
        complete!(cmd, "-D [TAB]"),
        snapbox::str![[r#"
comma
space
tab
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-D=[TAB]"),
        snapbox::str![[r#"
-D=comma
-D=space
-D=tab
"#]]
    );

    assert_data_eq!(complete!(cmd, "-D c[TAB]"), snapbox::str!["comma"]);

    assert_data_eq!(complete!(cmd, "-D=c[TAB]"), snapbox::str!["-D=comma"]);

    assert_data_eq!(
        complete!(cmd, "-D comma,[TAB]"),
        snapbox::str![[r#"
comma,comma
comma,space
comma,tab
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-D=comma,[TAB]"),
        snapbox::str![[r#"
-D=comma,comma
-D=comma,space
-D=comma,tab
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-D comma,s[TAB]"),
        snapbox::str!["comma,space"]
    );

    assert_data_eq!(
        complete!(cmd, "-D=comma,s[TAB]"),
        snapbox::str!["-D=comma,space"]
    );

    assert_data_eq!(
        complete!(cmd, "-- [TAB]"),
        snapbox::str![[r#"
a_pos
b_pos
c_pos
"#]]
    );

    assert_data_eq!(
        complete!(cmd, " -- a_pos,[TAB]"),
        snapbox::str![[r#"
a_pos,a_pos
a_pos,b_pos
a_pos,c_pos
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-- a_pos,b[TAB]"),
        snapbox::str!["a_pos,b_pos"]
    );
}

#[test]
fn suggest_delimited_multi_value_segments() {
    // `--format` takes at most two comma-separated values from a fixed set and
    // conflicts with `--raw`; completion of the segment under the cursor must
    // distinguish it from the segments already closed.
    fn command() -> Command {
        Command::new("fmt")
            .arg(
                clap::Arg::new("format")
                    .long("format")
                    .visible_alias("fmt-kind")
                    .short('f')
                    .num_args(1..=2)
                    .value_parser([
                        PossibleValue::new("json"),
                        PossibleValue::new("yaml"),
                        PossibleValue::new("toml"),
                    ])
                    .value_delimiter(','),
            )
            .arg(
                clap::Arg::new("raw")
                    .long("raw")
                    .visible_alias("unformatted")
                    .short('r')
                    .action(clap::ArgAction::SetTrue)
                    .conflicts_with("format"),
            )
            .arg(
                clap::Arg::new("verbose")
                    .long("verbose")
                    .short('v')
                    .action(clap::ArgAction::SetTrue),
            )
    }

    // A closed `--format=json,` keeps accepting the second segment on the next
    // empty word; `json` is already selected and the conflicting `--raw` (name,
    // alias and short) stays suppressed.  Because the dangling comma makes the
    // next segment mandatory, no unrelated option is offered there.
    assert_data_eq!(
        complete!(command(), "--format=json, [TAB]"),
        snapbox::str![[r#"
yaml
toml
"#]]
    );

    // Editing the same word filters the second segment on the text after the
    // last comma only; the unfinished word has not changed the conflict set
    // beyond recording `format`.
    assert_data_eq!(
        complete!(command(), "--format=json,y[TAB]"),
        snapbox::str!["--format=json,yaml"]
    );

    // The already selected `json` is never offered again, including via alias
    // and short spellings.
    assert_data_eq!(
        complete!(command(), "--format=json,j[TAB]"),
        snapbox::str![""]
    );
    assert_data_eq!(
        complete!(command(), "--fmt-kind=json,[TAB]"),
        snapbox::str![[r#"
--fmt-kind=json,yaml
--fmt-kind=json,toml
"#]]
    );
    assert_data_eq!(
        complete!(command(), "-fjson,[TAB]"),
        snapbox::str![[r#"
-fjson,yaml
-fjson,toml
"#]]
    );
    assert_data_eq!(
        complete!(command(), "-f=json,[TAB]"),
        snapbox::str![[r#"
-f=json,yaml
-f=json,toml
"#]]
    );

    // Space-separated value words reach the same selected state.
    assert_data_eq!(
        complete!(command(), "--format json, [TAB]"),
        snapbox::str![[r#"
yaml
toml
"#]]
    );

    // Once two values are closed, value taking ends and ordinary option
    // completion returns, with `raw` still suppressed.
    assert_data_eq!(
        complete!(command(), "--format=json,yaml [TAB]"),
        snapbox::str![[r#"
--format
--verbose
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(command(), "--format json,yaml [TAB]"),
        snapbox::str![[r#"
--format
--verbose
--help	Print help
"#]]
    );

    // A dangling comma after the last segment, or a third segment, cannot be
    // part of a valid command line: completion errors rather than guessing.
    for input in [
        "--format=json,yaml, [TAB]",
        "--format=json,yaml,toml [TAB]",
        "--format=json,yaml, --unformatted[TAB]",
        "--format=json,yaml, junk[TAB]",
    ] {
        assert_eq!(complete_err(&mut command(), input), "no completion generated");
    }

    // Editing the overfull word itself is illegal as well.
    for input in [
        "--format=json,yaml,[TAB]",
        "--format=json,yaml,t[TAB]",
        "--format=json,,[TAB]",
    ] {
        assert_eq!(complete_err(&mut command(), input), "no completion generated");
    }

    // Empty value segments are never silently accepted.
    for input in [
        "--format=,json[TAB]",
        "--format=,[TAB]",
        "--format=json,,y[TAB]",
        "-f=json,,[TAB]",
    ] {
        assert_eq!(complete_err(&mut command(), input), "no completion generated");
    }

    // A closed word carrying an unknown value cannot be part of a valid
    // command line, whichever spelling carried it.
    for input in [
        "--format=junk, [TAB]",
        "--format=junk [TAB]",
        "--format junk [TAB]",
        "--format json,junk [TAB]",
        "--fmt-kind=junk, [TAB]",
        "-fjson,junk [TAB]",
    ] {
        assert_eq!(complete_err(&mut command(), input), "no completion generated");
    }

    // An unknown closed segment inside the word being edited is illegal too;
    // only the segment under the cursor is filtered.
    for input in ["--format=junk,[TAB]", "--format=junk,y[TAB]"] {
        assert_eq!(complete_err(&mut command(), input), "no completion generated");
    }

    // An unfinished single segment keeps filtering like any value, and an
    // unknown prefix simply has no candidate.
    assert_data_eq!(
        complete!(command(), "--format=z[TAB]"),
        snapbox::str![""]
    );
    assert_data_eq!(
        complete!(command(), "--format=json,z[TAB]"),
        snapbox::str![""]
    );

    // The established state survives `--`; only positionals are completed
    // afterwards (there are none here), even after a dangling comma.
    assert_data_eq!(
        complete!(command(), "--format=json,yaml -- [TAB]"),
        snapbox::str![""]
    );
    assert_eq!(
        complete_err(&mut command(), "--format=json, -- [TAB]"),
        "no completion generated"
    );
}

#[test]
fn suggest_delimited_multi_value_conflict_suppression() {
    fn command() -> Command {
        Command::new("fmt")
            .arg(
                clap::Arg::new("format")
                    .long("format")
                    .num_args(1..=2)
                    .value_parser(["json", "yaml"])
                    .value_delimiter(','),
            )
            .arg(
                clap::Arg::new("raw")
                    .long("raw")
                    .action(clap::ArgAction::SetTrue)
                    .conflicts_with("format"),
            )
    }

    // While the second segment is open, typing the conflicting option offers
    // nothing in the dangling-segment value position.
    assert_data_eq!(
        complete!(command(), "--format=json, --r[TAB]"),
        snapbox::str![""]
    );

    // With the occurrence complete, the conflicting option name is suppressed,
    // whether the values arrived through `=` or separate words.
    assert_data_eq!(
        complete!(command(), "--format=json,yaml --r[TAB]"),
        snapbox::str![""]
    );
    assert_data_eq!(
        complete!(command(), "--format json,yaml --r[TAB]"),
        snapbox::str![""]
    );
    assert_data_eq!(
        complete!(command(), "--format=json --r[TAB]"),
        snapbox::str![""]
    );
}

#[test]
fn suggest_delimited_multi_value_alias_short_and_positional() {
    // The canonical name, its visible alias and its short all resolve to the
    // same option state; after `--` only the positional values are offered.
    fn command() -> Command {
        Command::new("fmt")
            .arg(
                clap::Arg::new("format")
                    .long("format")
                    .visible_alias("fmt")
                    .short('f')
                    .num_args(1..=2)
                    .value_parser(["json", "yaml", "toml"])
                    .value_delimiter(','),
            )
            .arg(
                clap::Arg::new("raw")
                    .long("raw")
                    .action(clap::ArgAction::SetTrue)
                    .conflicts_with("format"),
            )
            .arg(clap::Arg::new("path").value_parser(["src", "dst"]))
    }

    // Every spelling of a closed first segment reaches the same selected
    // state: only the unused values are offered, `raw` stays suppressed, and
    // the candidate keeps the caller's spelling.
    assert_data_eq!(
        complete!(command(), "--fmt=json, [TAB]"),
        snapbox::str![[r#"
yaml
toml
"#]]
    );
    assert_data_eq!(
        complete!(command(), "-fjson, [TAB]"),
        snapbox::str![[r#"
yaml
toml
"#]]
    );
    assert_data_eq!(
        complete!(command(), "--fmt=json,[TAB]"),
        snapbox::str![[r#"
--fmt=json,yaml
--fmt=json,toml
"#]]
    );

    // The word being edited filters on its own segment only and does not
    // change the conflict set.
    assert_data_eq!(
        complete!(command(), "--fmt=j[TAB]"),
        snapbox::str!["--fmt=json"]
    );
    assert_data_eq!(
        complete!(command(), "--fmt=json,y[TAB]"),
        snapbox::str!["--fmt=json,yaml"]
    );

    // Unknown closed values fail through every spelling.
    for input in ["--fmt=junk, [TAB]", "-fjunk [TAB]", "--format junk, [TAB]"] {
        assert_eq!(complete_err(&mut command(), input), "no completion generated");
    }

    // After `--` only the positional values remain.
    assert_data_eq!(
        complete!(command(), "--format=json,yaml -- [TAB]"),
        snapbox::str![[r#"
src
dst
"#]]
    );
    assert_data_eq!(
        complete!(command(), "-- s[TAB]"),
        snapbox::str!["src"]
    );
}

/// Build the command from the task spec: a two-value comma-delimited
/// `--format` (visible alias + short) with both a default and an env source,
/// a conflicting `--raw` (visible alias + short) and a directory positional.
fn spec_command() -> Command {
    Command::new("fmt")
        .arg(
            clap::Arg::new("format")
                .long("format")
                .visible_alias("fmt-kind")
                .short('f')
                .num_args(1..=2)
                .value_parser(["json", "yaml"])
                .value_delimiter(',')
                .default_value("json")
                .env("FMT_FORMAT_VALUE"),
        )
        .arg(
            clap::Arg::new("raw")
                .long("raw")
                .visible_alias("unformatted")
                .short('r')
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("format"),
        )
        .arg(clap::Arg::new("dir").value_hint(clap::ValueHint::DirPath))
}

fn spec_tempdir() -> snapbox::dir::DirRoot {
    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let path = testdir.path().unwrap();
    fs::create_dir_all(path.join("a_dir/nested")).unwrap();
    fs::create_dir_all(path.join("b_dir")).unwrap();
    fs::write(path.join("a_file"), "").unwrap();
    testdir
}

#[test]
fn suggest_implicit_sources_do_not_select_format() {
    // Neither the default value nor the env value is a user choice; an empty
    // word must still offer `format` (canonical name, visible alias, short),
    // `raw` (canonical name, visible alias, short) and help.
    assert_data_eq!(
        complete!(spec_command(), " [TAB]"),
        snapbox::str![[r#"
--format
--raw
--help	Print help
"#]]
    );

    // The same with an env value available: implicit sources never suppress.
    // SAFETY: this variable is unique to this test and the completion engine
    // never reads the environment, so nothing can observe a racy value.
    unsafe {
        std::env::set_var("FMT_FORMAT_VALUE", "json");
    }
    assert_data_eq!(
        complete!(spec_command(), " [TAB]"),
        snapbox::str![[r#"
--format
--raw
--help	Print help
"#]]
    );

    // Prefix completion surfaces the visible aliases too.
    assert_data_eq!(
        complete!(spec_command(), "--fmt[TAB]"),
        snapbox::str!["--fmt-kind"]
    );
    assert_data_eq!(
        complete!(spec_command(), "--unf[TAB]"),
        snapbox::str!["--unformatted"]
    );
    assert_data_eq!(
        complete!(spec_command(), "-[TAB]"),
        snapbox::str![[r#"
-f	--format
-r	--raw
-h	Print help
"#]]
    );
}

#[test]
fn suggest_explicit_format_conflicts_consistently() {
    // Every spelling of an explicit `format` establishes the same conflict:
    // the second segment is offered while open; after the occurrence closes
    // `format` stays but `raw` (name, visible alias and short) is suppressed.
    for input in ["--format json, [TAB]", "--fmt-kind json, [TAB]", "-f json, [TAB]"] {
        assert_data_eq!(
            complete!(spec_command(), input),
            snapbox::str![[r#"
yaml
"#]]
        );
    }

    assert_data_eq!(
        complete!(spec_command(), "--format=json,yaml [TAB]"),
        snapbox::str![[r#"
--format
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(spec_command(), "--fmt-kind=json,yaml [TAB]"),
        snapbox::str![[r#"
--format
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(spec_command(), "-fjson,yaml [TAB]"),
        snapbox::str![[r#"
--format
--help	Print help
"#]]
    );

    // The conflicting long and alias candidates stay hidden under every
    // spelling.
    for input in [
        "--format json,yaml --r[TAB]",
        "--fmt-kind=json,yaml --unf[TAB]",
    ] {
        assert_data_eq!(complete!(spec_command(), input), snapbox::str![""]);
    }
    // A short prefix keeps the typed cluster and offers the still-visible
    // flags behind it (the same existing semantics as any conflict).
    assert_data_eq!(
        complete!(spec_command(), "-fjson,yaml -r[TAB]"),
        snapbox::str![[r#"
-rf	--format
-rh	Print help
"#]]
    );

    // Already selected value does not reappear; the editing segment filters on
    // its own prefix only.
    assert_data_eq!(complete!(spec_command(), "--format=json,j[TAB]"), snapbox::str![""]);
    assert_data_eq!(
        complete!(spec_command(), "--format=json,y[TAB]"),
        snapbox::str!["--format=json,yaml"]
    );

    // Reverse direction: an explicit `raw` suppresses `format`.
    for input in ["--raw [TAB]", "--unformatted [TAB]", "-r [TAB]"] {
        assert_data_eq!(
            complete!(spec_command(), input),
            snapbox::str![[r#"
--raw
--help	Print help
"#]]
        );
    }
}

#[test]
fn suggest_only_directory_positionals_after_escape() {
    let testdir = spec_tempdir();
    let path = testdir.path().unwrap();

    // Conflict state established before `--` survives, but the output contains
    // only the directory positional's candidates (read from current_dir).
    assert_data_eq!(
        complete!(spec_command(), "--format json,yaml -- [TAB]", current_dir = Some(path)),
        snapbox::str![[r#"
.
a_dir/
b_dir/
"#]]
    );
    assert_data_eq!(
        complete!(spec_command(), "-fjson,yaml -- [TAB]", current_dir = Some(path)),
        snapbox::str![[r#"
.
a_dir/
b_dir/
"#]]
    );

    // Path prefix rules: partial name, an entered directory and `.`.
    assert_data_eq!(
        complete!(spec_command(), "--format json -- a[TAB]", current_dir = Some(path)),
        snapbox::str!["a_dir/"]
    );
    assert_data_eq!(
        complete!(spec_command(), "--format json -- a_dir/[TAB]", current_dir = Some(path)),
        snapbox::str!["a_dir/nested/"]
    );
    assert_data_eq!(
        complete!(spec_command(), "--format json -- .[TAB]", current_dir = Some(path)),
        snapbox::str![[r#"
./a_dir/
./b_dir/
"#]]
    );

    // No option, alias or option value leaks after `--`.
    assert_data_eq!(
        complete!(spec_command(), "--format json -- --r[TAB]", current_dir = Some(path)),
        snapbox::str![""]
    );
    assert_data_eq!(
        complete!(spec_command(), "--format json -- --fmt[TAB]", current_dir = Some(path)),
        snapbox::str![""]
    );
    assert_data_eq!(
        complete!(spec_command(), "--raw -- [TAB]", current_dir = Some(path)),
        snapbox::str![[r#"
.
a_dir/
b_dir/
"#]]
    );
}

#[test]
fn suggest_dedups_same_insertion_across_sources() {
    // A subcommand and a positional possible value propose the same text; the
    // literal insertion appears once regardless of the source/order.
    let mut cmd = Command::new("fmt")
        .subcommand(Command::new("src"))
        .arg(clap::Arg::new("dir").value_parser(["src", "dst"]));

    assert_data_eq!(
        complete!(cmd, " [TAB]"),
        snapbox::str![[r#"
src
help	Print this message or the help of the given subcommand(s)
dst
--help	Print help
"#]]
    );

    // A path candidate with the same insertion text as a subcommand candidate
    // is kept once as well (here an `AnyPath` entry named like a subcommand).
    let testdir = snapbox::dir::DirRoot::mutable_temp().unwrap();
    let path = testdir.path().unwrap();
    fs::write(path.join("dup"), "").unwrap();
    fs::write(path.join("other"), "").unwrap();
    let mut cmd = Command::new("fmt")
        .subcommand(Command::new("dup"))
        .arg(clap::Arg::new("path").value_hint(clap::ValueHint::AnyPath));
    assert_data_eq!(
        complete!(cmd, " [TAB]", current_dir = Some(path)),
        snapbox::str![[r#"
dup
help	Print this message or the help of the given subcommand(s)
.
other
--help	Print help
"#]]
    );
}

#[test]
fn complete_first_word_respects_binary_name_setting() {
    let testdir = spec_tempdir();
    let path = testdir.path().unwrap();

    // Default with a program name: args[0] is skipped, never mistaken for an
    // argument or the positional's value, so the positional's directory
    // candidates are still offered and nothing is suppressed.
    let mut cmd = spec_command();
    let completions = complete_values(&mut cmd, vec!["fmt".into(), "".into()], 1, Some(path));
    assert_eq!(
        completions,
        vec![
            ".".to_owned(),
            "a_dir/".to_owned(),
            "b_dir/".to_owned(),
            "--format".to_owned(),
            "--raw".to_owned(),
            "--help".to_owned(),
        ]
    );

    // no_binary_name(true) with the same word sequence: `fmt` is parsed as
    // the positional's value, leaving no positional to complete.
    let mut cmd = spec_command().no_binary_name(true);
    let completions = complete_values(&mut cmd, vec!["fmt".into(), "".into()], 1, Some(path));
    assert_eq!(
        completions,
        vec!["--format".to_owned(), "--raw".to_owned(), "--help".to_owned()]
    );

    // no_binary_name(true) without a program name: an option in first
    // position records state from the very first word, so `--format json`
    // selects format and suppresses raw.
    let mut cmd = spec_command().no_binary_name(true);
    let completions = complete_values(
        &mut cmd,
        vec!["--format".into(), "json".into(), "".into()],
        2,
        None,
    );
    assert!(completions.contains(&"yaml".to_owned()));
    assert!(completions.contains(&"--format".to_owned()));
    assert!(!completions.contains(&"--raw".to_owned()));

    // Default without a program name: `--format` itself is skipped as the
    // binary name, so nothing is recorded and `raw` stays available.
    let mut cmd = spec_command();
    let completions = complete_values(
        &mut cmd,
        vec!["--format".into(), "json".into(), "".into()],
        2,
        None,
    );
    assert!(completions.contains(&"--format".to_owned()));
    assert!(completions.contains(&"--raw".to_owned()));
}

#[test]
fn suggest_spec_illegal_forms_error() {
    // Unknown values, empty segments, a third segment and illegal `=` forms
    // keep reporting the existing error rather than guessing a candidate from
    // the default, the env value or the word being edited.
    for input in [
        "--format=junk [TAB]",
        "--format junk [TAB]",
        "--format=json,,y[TAB]",
        "--format=json,yaml, [TAB]",
        "--format=json,yaml,toml [TAB]",
        "--format=json= [TAB]",
        "--format=json, -- [TAB]",
    ] {
        assert_eq!(complete_err(&mut spec_command(), input), "no completion generated");
    }

    // A closed `--raw=x` word keeps the existing flag-recording semantics:
    // `raw` is present, so `format` stays suppressed (it is not an error).
    assert_data_eq!(
        complete!(spec_command(), "--raw=x [TAB]"),
        snapbox::str![[r#"
--raw
--help	Print help
"#]]
    );
}

/// Build the command from the terminator spec: `--tag` with a visible alias
/// `--label`, short `-t`, at most three comma-separated values from a fixed
/// set ended by a standalone `;`; a conflicting value-less `--raw`; and a
/// positional limited to `src`/`dst` that also completes directories.
fn tag_command() -> Command {
    Command::new("fmt")
        .arg(
            clap::Arg::new("tag")
                .long("tag")
                .visible_alias("label")
                .short('t')
                .num_args(1..=3)
                .value_parser(["red", "green", "blue"])
                .value_delimiter(',')
                .value_terminator(";"),
        )
        .arg(
            clap::Arg::new("raw")
                .long("raw")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("tag"),
        )
        .arg(
            clap::Arg::new("path")
                .value_parser(["src", "dst"])
                .value_hint(clap::ValueHint::DirPath),
        )
}

#[test]
fn suggest_tag_terminator_open_segments() {
    // A dangling comma before the cursor keeps accepting segments: only the
    // unused values are offered, never the selected `red`, option names or
    // positionals. Every spelling of the option establishes the same state and
    // keeps the prefix the caller is typing.
    for input in [
        "--tag=red, [TAB]",
        "--label=red, [TAB]",
        "-tred, [TAB]",
        "-t=red, [TAB]",
        "--tag red, [TAB]",
    ] {
        assert_data_eq!(
            complete!(tag_command(), input),
            snapbox::str![[r#"
green
blue
"#]]
        );
    }

    // The segment being edited filters on its own prefix only and keeps the
    // exact option spelling; the unfinished word does not change any other set.
    assert_data_eq!(
        complete!(tag_command(), "--tag=red,g[TAB]"),
        snapbox::str!["--tag=red,green"]
    );
    assert_data_eq!(
        complete!(tag_command(), "--label=red,g[TAB]"),
        snapbox::str!["--label=red,green"]
    );
    assert_data_eq!(
        complete!(tag_command(), "-tred,b[TAB]"),
        snapbox::str!["-tred,blue"]
    );

    // An already selected value never reappears, in any segment.
    assert_data_eq!(complete!(tag_command(), "--tag=red,r[TAB]"), snapbox::str![""]);
    assert_data_eq!(
        complete!(tag_command(), "--tag red,green,b[TAB]"),
        snapbox::str!["red,green,blue"]
    );
}

#[test]
fn suggest_tag_terminator_transition() {
    // A consumed standalone `;` ends value taking: it is not a value and is not a
    // candidate. The next empty word offers ordinary options and positionals
    // again, with `raw` still suppressed.
    assert_data_eq!(
        complete!(tag_command(), "--tag red ; [TAB]"),
        snapbox::str![[r#"
src
dst
--tag
--help	Print help
"#]]
    );
    // The same state through two space-separated value words and an alias.
    assert_data_eq!(
        complete!(tag_command(), "--tag red green ; [TAB]"),
        snapbox::str![[r#"
src
dst
--tag
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(tag_command(), "--label red ; [TAB]"),
        snapbox::str![[r#"
src
dst
--tag
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(tag_command(), "-t red ; [TAB]"),
        snapbox::str![[r#"
src
dst
--tag
--help	Print help
"#]]
    );

    // While the cursor sits on the terminator itself nothing is offered: it is
    // a transition word, not a value, option or positional.
    assert_data_eq!(
        complete!(tag_command(), "--tag red ;[TAB]"),
        snapbox::str![""]
    );

    // `raw` stays suppressed after the transition: its long prefix offers
    // nothing, and its short prefix only clusters the still-visible flags.
    assert_data_eq!(complete!(tag_command(), "--tag red ; --r[TAB]"), snapbox::str![""]);
    assert_data_eq!(
        complete!(tag_command(), "--tag red ; -r[TAB]"),
        snapbox::str![[r#"
-rt	--tag
-rh	Print help
"#]]
    );
}

#[test]
fn suggest_tag_terminator_then_escape() {
    let testdir = spec_tempdir();
    let path = testdir.path().unwrap();

    // After the terminator and `--`, only the positional's fixed values and
    // directory candidates (sorted by the existing rules) remain.
    assert_data_eq!(
        complete!(tag_command(), "--tag red ; -- [TAB]", current_dir = Some(path)),
        snapbox::str![[r#"
src
dst
.
a_dir/
b_dir/
"#]]
    );
    assert_data_eq!(
        complete!(tag_command(), "--tag red ; -- s[TAB]", current_dir = Some(path)),
        snapbox::str!["src"]
    );
    assert_data_eq!(
        complete!(tag_command(), "--tag red ; -- a[TAB]", current_dir = Some(path)),
        snapbox::str!["a_dir/"]
    );
    // No option leaks past `--`, and `raw` stays suppressed there too.
    assert_data_eq!(
        complete!(tag_command(), "--tag red ; -- --r[TAB]", current_dir = Some(path)),
        snapbox::str![""]
    );
}

#[test]
fn suggest_tag_terminator_illegal_forms() {
    // The terminator needs at least the required values: a bare `--tag ;`
    // cannot end an empty occurrence.
    assert_eq!(
        complete_err(&mut tag_command(), "--tag ; [TAB]"),
        "no completion generated"
    );

    // A dangling comma opened an empty segment that `;` cannot close.
    for input in ["--tag=red, ; [TAB]", "-tred, ; [TAB]", "--tag red, ; [TAB]"] {
        assert_eq!(complete_err(&mut tag_command(), input), "no completion generated");
    }

    // Unknown values and overfull value sets stay illegal.
    for input in [
        "--tag junk [TAB]",
        "--tag red junk [TAB]",
        "--tag=junk [TAB]",
        "--tag=red,green,blue,[TAB]",
        "--tag red,, [TAB]",
        "--tag=red,,;[TAB]",
    ] {
        assert_eq!(complete_err(&mut tag_command(), input), "no completion generated");
    }

    // A repeated standalone terminator and a terminator glued to other text
    // after the transition cannot be guessed at.
    for input in [
        "--tag red ; ; [TAB]",
        "--tag red ; ;[TAB]",
        "--tag red ; ;x[TAB]",
        "--tag red ; x;[TAB]",
    ] {
        assert_eq!(complete_err(&mut tag_command(), input), "no completion generated");
    }

    // A terminator glued into a value word is neither a value nor a
    // terminator, whichever spelling carries it, whether it is a separate word
    // or still part of the option word under the cursor.
    for input in [
        "--tag red; [TAB]",
        "--tag=red; [TAB]",
        "-tred; [TAB]",
        "--label=red; [TAB]",
        "--tag=red;[TAB]",
        "-tred;[TAB]",
        "--tag=red;x[TAB]",
    ] {
        assert_eq!(complete_err(&mut tag_command(), input), "no completion generated");
    }
}

fn complete_values(
    cmd: &mut Command,
    args: Vec<std::ffi::OsString>,
    arg_index: usize,
    current_dir: Option<&Path>,
) -> Vec<String> {
    clap_complete::engine::complete(cmd, args, arg_index, current_dir)
        .unwrap()
        .into_iter()
        .map(|c| c.get_value().to_str().unwrap().to_owned())
        .collect()
}

fn complete_err(cmd: &mut Command, args: impl AsRef<str>) -> String {
    let input = args.as_ref();
    let mut raw = vec![std::ffi::OsString::from(cmd.get_name())];
    if let Some((prior, _after)) = input.split_once("[TAB]") {
        raw.extend(prior.split_whitespace().map(From::from));
        if prior.ends_with(char::is_whitespace) {
            raw.push(std::ffi::OsString::default());
        }
    }
    let arg_index = raw.len() - 1;
    clap_complete::engine::complete(cmd, raw, arg_index, None)
        .unwrap_err()
        .to_string()
}

#[test]
fn suggest_allow_hyphen() {
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("format")
                .long("format")
                .short('F')
                .allow_hyphen_values(true)
                .value_parser(["--json", "--toml", "--yaml"]),
        )
        .arg(clap::Arg::new("json").long("json"));

    assert_data_eq!(complete!(cmd, "--format --j[TAB]"), snapbox::str!["--json"]);
    assert_data_eq!(complete!(cmd, "-F --j[TAB]"), snapbox::str!["--json"]);
    assert_data_eq!(complete!(cmd, "--format --t[TAB]"), snapbox::str!["--toml"]);
    assert_data_eq!(complete!(cmd, "-F --t[TAB]"), snapbox::str!["--toml"]);

    assert_data_eq!(
        complete!(cmd, "--format --[TAB]"),
        snapbox::str![[r#"
--json
--toml
--yaml
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "-F --[TAB]"),
        snapbox::str![[r#"
--json
--toml
--yaml
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format --json --j[TAB]"),
        snapbox::str!["--json"]
    );

    assert_data_eq!(
        complete!(cmd, "-F --json --j[TAB]"),
        snapbox::str!["--json"]
    );
}

#[test]
fn suggest_positional_long_allow_hyphen() {
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("format")
                .long("format")
                .short('F')
                .allow_hyphen_values(true)
                .value_parser(["--json", "--toml", "--yaml"]),
        )
        .arg(
            clap::Arg::new("positional_a")
                .value_parser(["--pos_a"])
                .allow_hyphen_values(true),
        )
        .arg(clap::Arg::new("positional_b").value_parser(["pos_b"]));

    assert_data_eq!(
        complete!(cmd, "--format --json --pos[TAB]"),
        snapbox::str!["--pos_a"]
    );
    assert_data_eq!(
        complete!(cmd, "-F --json --pos[TAB]"),
        snapbox::str!["--pos_a"]
    );

    assert_data_eq!(
        complete!(cmd, "--format --json --pos_a [TAB]"),
        snapbox::str![[r#"
pos_b
--format
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "-F --json --pos_a [TAB]"),
        snapbox::str![[r#"
pos_b
--format
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format --json --pos_a p[TAB]"),
        snapbox::str!["pos_b"]
    );
    assert_data_eq!(
        complete!(cmd, "-F --json --pos_a p[TAB]"),
        snapbox::str!["pos_b"]
    );
}

#[test]
fn suggest_positional_short_allow_hyphen() {
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("format")
                .long("format")
                .short('F')
                .allow_hyphen_values(true)
                .value_parser(["--json", "--toml", "--yaml"]),
        )
        .arg(
            clap::Arg::new("positional_a")
                .value_parser(["-a"])
                .allow_hyphen_values(true),
        )
        .arg(clap::Arg::new("positional_b").value_parser(["pos_b"]));

    assert_data_eq!(
        complete!(cmd, "--format --json -a [TAB]"),
        snapbox::str![[r#"
pos_b
--format
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "-F --json -a [TAB]"),
        snapbox::str![[r#"
pos_b
--format
--help	Print help
"#]]
    );

    assert_data_eq!(
        complete!(cmd, "--format --json -a p[TAB]"),
        snapbox::str!["pos_b"]
    );
    assert_data_eq!(
        complete!(cmd, "-F --json -a p[TAB]"),
        snapbox::str!["pos_b"]
    );
}

#[test]
fn suggest_external_subcommand() {
    let mut cmd = Command::new("dynamic")
        .allow_external_subcommands(true)
        .add(SubcommandCandidates::new(|| {
            vec![CompletionCandidate::new("external")]
        }))
        .arg(clap::Arg::new("positional").value_parser(["pos1", "pos2", "pos3"]));

    assert_data_eq!(
        complete!(cmd, " [TAB]"),
        snapbox::str![
            "external
pos1
pos2
pos3
--help\tPrint help
"
        ]
    );

    assert_data_eq!(complete!(cmd, "e[TAB]"), snapbox::str!["external"]);
}

#[test]
fn sort_and_filter() {
    let mut cmd = Command::new("exhaustive")
        .args([
            clap::Arg::new("required-flag")
                .long("required-flag")
                .visible_alias("required-flag2")
                .short('r')
                .required(true),
            clap::Arg::new("optional-flag")
                .long("optional-flag")
                .visible_alias("2optional-flag")
                .short('o'),
            clap::Arg::new("long-flag").long("long-flag"),
            clap::Arg::new("short-flag").short('s'),
            clap::Arg::new("positional").value_parser(["pos-a", "pos-b", "pos-c"]),
        ])
        .subcommands([Command::new("sub")]);

    assert_data_eq!(
        complete!(cmd, " [TAB]"),
        snapbox::str![[r#"
sub
help	Print this message or the help of the given subcommand(s)
pos-a
pos-b
pos-c
--required-flag
--optional-flag
--long-flag
-s
--help	Print help
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "-[TAB]"),
        snapbox::str![[r#"
-r	--required-flag
-o	--optional-flag
--long-flag
-s
-h	Print help
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "--[TAB]"),
        snapbox::str![[r#"
--required-flag
--optional-flag
--long-flag
--help	Print help
"#]]
    );
}

#[test]
fn suggest_conflicting_args() {
    fn command() -> Command {
        Command::new("exhaustive")
            .arg(
                clap::Arg::new("safe")
                    .long("safe")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("fast")
                    .long("fast")
                    .visible_alias("quick")
                    .alias("speedy")
                    .action(clap::ArgAction::SetTrue)
                    .conflicts_with("safe"),
            )
            .arg(
                clap::Arg::new("tag")
                    .long("tag")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("file")
                    .long("file")
                    .action(clap::ArgAction::SetTrue),
            )
            .group(
                clap::ArgGroup::new("payload")
                    .args(["tag", "file"])
                    .conflicts_with("fast"),
            )
    }

    assert_data_eq!(
        complete!(command(), "--fast [TAB]"),
        snapbox::str![[r#"
--fast
--help	Print help
"#]],
    );

    assert_data_eq!(
        complete!(command(), "--tag [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );

    // Conflicts are bidirectional: `--safe` hides `--fast` and its aliases,
    // while the unrelated `payload` group stays available.
    assert_data_eq!(
        complete!(command(), "--safe [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--file
--help	Print help
"#]],
    );

    // A non-`multiple` group makes its members conflict with each other.
    assert_data_eq!(
        complete!(command(), "--file [TAB]"),
        snapbox::str![[r#"
--safe
--file
--help	Print help
"#]],
    );

    // Prefix-completion keeps the present argument itself, including its
    // aliases; only disabled arguments lose theirs.
    assert_data_eq!(
        complete!(command(), "--fast --q[TAB]"),
        snapbox::str!["--quick"]
    );
    assert_data_eq!(
        complete!(command(), "--fast --spe[TAB]"),
        snapbox::str!["--speedy"]
    );
    assert_data_eq!(
        complete!(command(), "--fast --ta[TAB]"),
        snapbox::str![]
    );

    // A value given to an option is not mistaken for an option name.
    assert_data_eq!(
        complete!(command(), "--tag --safe [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );
}

#[test]
fn suggest_conflicting_short_flags() {
    // The short spellings must produce exactly the same conflict results as
    // the long ones: `-f/--fast` conflicts with `-s/--safe`, `-t/--tag` takes
    // a value and conflicts with `fast`.
    fn command() -> Command {
        Command::new("exhaustive")
            .arg(
                clap::Arg::new("fast")
                    .long("fast")
                    .short('f')
                    .visible_alias("quick")
                    .alias("speedy")
                    .action(clap::ArgAction::SetTrue)
                    .conflicts_with("safe"),
            )
            .arg(
                clap::Arg::new("safe")
                    .long("safe")
                    .short('s')
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("tag")
                    .long("tag")
                    .short('t')
                    .value_parser(["v1", "v2"])
                    .conflicts_with("fast"),
            )
    }

    // `tag` given as a separate value word hides `fast` (long, short and
    // aliases) while keeping `tag` itself and `help`.
    assert_data_eq!(
        complete!(command(), "-t v1 [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );

    // The attached value form has the identical result.
    assert_data_eq!(
        complete!(command(), "-tv1 [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );
    assert_data_eq!(
        complete!(command(), "-t=v1 [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );

    // Parity with the long spellings, both separate and attached.
    assert_data_eq!(
        complete!(command(), "--tag v1 [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );
    assert_data_eq!(
        complete!(command(), "--tag=v1 [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );

    // `fast` hides `safe`, `tag` and itself stays.
    assert_data_eq!(
        complete!(command(), "-f [TAB]"),
        snapbox::str![[r#"
--fast
--help	Print help
"#]],
    );
    assert_data_eq!(
        complete!(command(), "-s [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );

    // Short candidates are suppressed as thoroughly as long ones.
    assert_data_eq!(
        complete!(command(), "-t v1 -[TAB]"),
        snapbox::str![[r#"
-s	--safe
-t	--tag
-h	Print help
"#]],
    );
    assert_data_eq!(
        complete!(command(), "-t v1 -f[TAB]"),
        snapbox::str![[r#"
-fs	--safe
-ft	--tag
-fh	Print help
"#]],
    );

    // Visible and hidden aliases of a suppressed argument vanish too.
    assert_data_eq!(complete!(command(), "-t v1 --q[TAB]"), snapbox::str![""]);
    assert_data_eq!(
        complete!(command(), "-t v1 --spe[TAB]"),
        snapbox::str![""]
    );
    assert_data_eq!(
        complete!(command(), "-t v1 --s[TAB]"),
        snapbox::str!["--safe"]
    );

    // While the value is still being edited, only the option's own values are
    // offered; conflict results from following options are not applied early.
    assert_data_eq!(
        complete!(command(), "-t [TAB]"),
        snapbox::str![[r#"
v1
v2
"#]],
    );
    assert_data_eq!(
        complete!(command(), "-tv[TAB]"),
        snapbox::str![[r#"
-tv1
-tv2
"#]],
    );

    // Every member of a legal cluster participates in conflict resolution:
    // `s` is recorded alongside the value-taking `t`, and `t` still hides
    // `fast`.
    assert_data_eq!(
        complete!(command(), "-st v1 [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );
    assert_data_eq!(
        complete!(command(), "-stv1 [TAB]"),
        snapbox::str![[r#"
--safe
--tag
--help	Print help
"#]],
    );

    // `f` before the value-taking `t` is recorded as well, so `safe` is hidden
    // by the time the attached value closes the word.
    assert_data_eq!(
        complete!(command(), "-ftv1 [TAB]"),
        snapbox::str![[r#"
--fast
--tag
--help	Print help
"#]],
    );

    // Invalid short strings keep the existing completion semantics: their
    // recognized members are not guessed out of the cluster and therefore do
    // not suppress anything.
    assert_data_eq!(
        complete!(command(), "-fx [TAB]"),
        snapbox::str![[r#"
--fast
--safe
--tag
--help	Print help
"#]],
    );
    assert_data_eq!(
        complete!(command(), "-xt v1 [TAB]"),
        snapbox::str![[r#"
--fast
--safe
--tag
--help	Print help
"#]],
    );

    // After `--` only positionals are completed (none here); state parsed from
    // the preceding short options stays valid.
    assert_data_eq!(complete!(command(), "-f -- [TAB]"), snapbox::str![""]);
}

#[test]
fn complete_no_binary_name_short_records_state() {
    fn command() -> Command {
        Command::new("exhaustive")
            .no_binary_name(true)
            .arg(
                clap::Arg::new("fast")
                    .long("fast")
                    .short('f')
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("tag")
                    .long("tag")
                    .short('t')
                    .value_parser(["v1", "v2"])
                    .conflicts_with("fast"),
            )
    }

    // With `no_binary_name`, the first word is parsed as an option.
    let mut cmd = command();
    let completions =
        clap_complete::engine::complete(&mut cmd, vec!["-t".into(), "v1".into(), "".into()], 2, None)
            .unwrap()
            .into_iter()
            .map(|c| c.get_value().to_str().unwrap().to_owned())
            .collect::<Vec<_>>();
    assert_eq!(completions, vec!["--tag", "--help"]);
}

#[test]
fn suggest_conflicts_declared_on_absent_arg() {
    // The conflict is declared by `safe`, yet completing after `--fast` must
    // still hide `--safe`: conflicts are evaluated bidirectionally.
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("fast")
                .long("fast")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("safe")
                .long("safe")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("fast"),
        );

    assert_data_eq!(
        complete!(cmd, "--fast [TAB]"),
        snapbox::str![[r#"
--fast
--help	Print help
"#]],
    );
}

#[test]
fn suggest_conflicts_with_all_and_groups() {
    let mut cmd = Command::new("exhaustive")
        .arg(clap::Arg::new("a").long("a").action(clap::ArgAction::SetTrue))
        .arg(clap::Arg::new("b").long("b").action(clap::ArgAction::SetTrue))
        .arg(clap::Arg::new("c").long("c").action(clap::ArgAction::SetTrue))
        .arg(
            clap::Arg::new("all")
                .long("all")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with_all(["a", "b", "c"]),
        );

    assert_data_eq!(
        complete!(cmd, "--all [TAB]"),
        snapbox::str![[r#"
--all
--help	Print help
"#]],
    );
}

#[test]
fn suggest_conflicts_unroll_group() {
    fn command() -> Command {
        Command::new("exhaustive")
            .arg(clap::Arg::new("a").long("a").action(clap::ArgAction::SetTrue))
            .arg(clap::Arg::new("b").long("b").action(clap::ArgAction::SetTrue))
            .arg(
                clap::Arg::new("other")
                    .long("other")
                    .action(clap::ArgAction::SetTrue),
            )
            .group(
                clap::ArgGroup::new("ab")
                    .args(["a", "b"])
                    .conflicts_with("other"),
            )
    }

    // A conflict with a group is expanded to every member: `--other` hides
    // `--a` and `--b`, in both directions.
    assert_data_eq!(
        complete!(command(), "--other [TAB]"),
        snapbox::str![[r#"
--other
--help	Print help
"#]],
    );

    // `--a` hides `--other` through the group and hides `--b` because the
    // group is not `multiple`.
    assert_data_eq!(
        complete!(command(), "--a [TAB]"),
        snapbox::str![[r#"
--a
--help	Print help
"#]],
    );

    // A multiple(true) group lets its members coexist.
    let mut cmd = Command::new("exhaustive")
        .arg(clap::Arg::new("a").long("a").action(clap::ArgAction::SetTrue))
        .arg(clap::Arg::new("b").long("b").action(clap::ArgAction::SetTrue))
        .group(clap::ArgGroup::new("ab").args(["a", "b"]).multiple(true));
    assert_data_eq!(
        complete!(cmd, "--a [TAB]"),
        snapbox::str![[r#"
--a
--b
--help	Print help
"#]],
    );
}

#[test]
fn suggest_keeps_overrides_with_candidates() {
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("color")
                .long("color")
                .action(clap::ArgAction::SetTrue)
                .overrides_with("mono"),
        )
        .arg(
            clap::Arg::new("mono")
                .long("mono")
                .action(clap::ArgAction::SetTrue),
        );

    // `overrides_with` is not a conflict: both options stay available.
    assert_data_eq!(
        complete!(cmd, "--color [TAB]"),
        snapbox::str![[r#"
--color
--mono
--help	Print help
"#]],
    );
}

#[test]
fn suggest_no_options_after_escape_with_conflicts() {
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("positional").value_parser(["pos-a", "pos-b", "pos-c"]),
        )
        .arg(
            clap::Arg::new("fast")
                .long("fast")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("safe")
                .long("safe")
                .action(clap::ArgAction::SetTrue)
                .conflicts_with("fast"),
        );

    // After `--` only positional arguments are completed, conflicts or not.
    assert_data_eq!(
        complete!(cmd, "--fast -- [TAB]"),
        snapbox::str![[r#"
pos-a
pos-b
pos-c
"#]],
    );
}

#[test]
fn suggest_long_equals_visible_alias_values() {
    // A value-taking option reached through a visible long alias must offer
    // the same attached-value candidates as through its canonical long name,
    // keeping the alias spelling as the candidate prefix.
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("fast")
                .long("fast")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("tag")
                .long("tag")
                .visible_alias("label")
                .alias("ticket")
                .value_parser(["red", "blue"])
                .conflicts_with("fast"),
        );

    assert_data_eq!(
        complete!(cmd, "--label=[TAB]"),
        snapbox::str![[r#"
--label=red
--label=blue
"#]]
    );
    assert_data_eq!(
        complete!(cmd, "--label=r[TAB]"),
        snapbox::str!["--label=red"]
    );

    // A closed attached value through the alias restores the same state as the
    // canonical spelling: `tag` is present and hides `fast`.
    assert_data_eq!(
        complete!(cmd, "--label=red [TAB]"),
        snapbox::str![[r#"
--tag
--help	Print help
"#]]
    );

    // Hidden aliases stay undiscoverable, including with an attached value.
    assert_data_eq!(complete!(cmd, "--ticket=r[TAB]"), snapbox::str![""]);
}

#[test]
fn complete_no_binary_name_keeps_first_arg() {
    fn command() -> Command {
        Command::new("exhaustive")
            .no_binary_name(true)
            .arg(
                clap::Arg::new("fast")
                    .long("fast")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                clap::Arg::new("safe")
                    .long("safe")
                    .action(clap::ArgAction::SetTrue)
                    .conflicts_with("fast"),
            )
    }

    let mut cmd = command();
    let completions =
        clap_complete::engine::complete(&mut cmd, vec!["--fast".into(), "".into()], 1, None)
            .unwrap()
            .into_iter()
            .map(|c| c.get_value().to_str().unwrap().to_owned())
            .collect::<Vec<_>>();
    assert_eq!(completions, vec!["--fast", "--help"]);

    // Without `no_binary_name`, `args[0]` is the binary name and skipped, so
    // `--fast` is not part of the completed command line.
    let mut cmd = command().no_binary_name(false);
    let completions =
        clap_complete::engine::complete(&mut cmd, vec!["--fast".into(), "".into()], 1, None)
            .unwrap()
            .into_iter()
            .map(|c| c.get_value().to_str().unwrap().to_owned())
            .collect::<Vec<_>>();
    assert!(completions.contains(&"--fast".to_owned()));
    assert!(completions.contains(&"--safe".to_owned()));
}

#[test]
fn complete_without_word_is_error() {
    let mut cmd = Command::new("exhaustive").arg(
        clap::Arg::new("fast")
            .long("fast")
            .action(clap::ArgAction::SetTrue),
    );
    let err = clap_complete::engine::complete(
        &mut cmd,
        vec![std::ffi::OsString::from("exhaustive"), "--fast".into()],
        2,
        None,
    )
    .unwrap_err();
    assert_eq!(err.to_string(), "no completion generated");
}

#[test]
fn suggest_long_equals_value_position() {
    // A `--name=value` word at the cursor is the option's value position:
    // only the option's own values are offered, exactly as if the value had
    // been written as a separate word.  Nothing past `=` is completed as an
    // option name, subcommand or positional.
    let mut cmd = Command::new("exhaustive")
        .arg(clap::Arg::new("tag").long("tag").value_parser(["red", "blue"]))
        .arg(clap::Arg::new("pos").value_parser(["pos-a", "pos-b"]))
        .subcommand(Command::new("sub"));

    assert_data_eq!(
        complete!(cmd, "--tag=[TAB]"),
        snapbox::str![[r#"
--tag=red
--tag=blue
"#]]
    );
    assert_data_eq!(complete!(cmd, "--tag=r[TAB]"), snapbox::str!["--tag=red"]);

    // A closed attached value ends the option's values; the next word is a
    // fresh position again.
    assert_data_eq!(
        complete!(cmd, "--tag=red [TAB]"),
        snapbox::str![[r#"
sub
help	Print this message or the help of the given subcommand(s)
pos-a
pos-b
--tag
--help	Print help
"#]]
    );

    // The state established through `=` survives `--`; only positionals are
    // completed afterwards.
    assert_data_eq!(
        complete!(cmd, "--tag=red -- [TAB]"),
        snapbox::str![[r#"
pos-a
pos-b
"#]]
    );
}

#[test]
fn suggest_long_equals_multi_values() {
    // For an option taking multiple values, every segment after `=` belongs
    // to that option until its value rule ends.
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("nums")
                .long("nums")
                .num_args(1..=3)
                .value_parser(["one", "two", "three"]),
        )
        .arg(clap::Arg::new("pos").value_parser(["pos-a"]));

    assert_data_eq!(
        complete!(cmd, "--nums=[TAB]"),
        snapbox::str![[r#"
--nums=one
--nums=two
--nums=three
"#]]
    );
    assert_data_eq!(complete!(cmd, "--nums=o[TAB]"), snapbox::str!["--nums=one"]);

    // An attached value closes the occurrence, mirroring `--nums one`.
    assert_data_eq!(
        complete!(cmd, "--nums=one [TAB]"),
        snapbox::str![[r#"
pos-a
--nums
--help	Print help
"#]]
    );
}

#[test]
fn suggest_long_equals_failure_semantics() {
    // Unknown long names, flags that take no value and unparseable words keep
    // the existing failure semantics instead of guessing a split.
    let mut cmd = Command::new("exhaustive")
        .arg(
            clap::Arg::new("fast")
                .long("fast")
                .action(clap::ArgAction::SetTrue),
        )
        .arg(
            clap::Arg::new("tag")
                .long("tag")
                .value_parser(["v1", "v2"])
                .conflicts_with("fast"),
        );

    assert_data_eq!(complete!(cmd, "--nope=[TAB]"), snapbox::str![""]);
    assert_data_eq!(complete!(cmd, "--fast=[TAB]"), snapbox::str![""]);

    // A closed `--fast=x` word keeps the existing behavior of recording
    // `fast`, so the conflicting `tag` stays suppressed.
    assert_data_eq!(
        complete!(cmd, "--fast=x [TAB]"),
        snapbox::str![[r#"
--fast
--help	Print help
"#]]
    );

    // The word being edited is not part of the conflict set yet, but
    // conflicts from earlier words still apply: a prior `--fast` disables
    // `tag`, so its value candidates are suppressed.
    assert_data_eq!(complete!(cmd, "--fast --tag=[TAB]"), snapbox::str![""]);

    // Without the conflicting `--fast`, the editing word itself does not
    // disable anything: `tag`'s own values are offered while `--tag=` is
    // being typed.
    assert_data_eq!(
        complete!(cmd, "--tag=[TAB]"),
        snapbox::str![[r#"
--tag=v1
--tag=v2
"#]]
    );
}

#[test]
fn complete_no_binary_name_equals_records_state() {
    let mut cmd = Command::new("exhaustive")
        .no_binary_name(true)
        .arg(
            clap::Arg::new("tag")
                .long("tag")
                .value_parser(["v1", "v2"])
                .conflicts_with("fast"),
        )
        .arg(
            clap::Arg::new("fast")
                .long("fast")
                .action(clap::ArgAction::SetTrue),
        );

    // With `no_binary_name`, a first word of `--tag=v1` records `tag` as
    // present and hides the conflicting `fast`.
    let completions =
        clap_complete::engine::complete(&mut cmd, vec!["--tag=v1".into(), "".into()], 1, None)
            .unwrap()
            .into_iter()
            .map(|c| c.get_value().to_str().unwrap().to_owned())
            .collect::<Vec<_>>();
    assert_eq!(completions, vec!["--tag", "--help"]);
}

fn complete(cmd: &mut Command, args: impl AsRef<str>, current_dir: Option<&Path>) -> String {
    let input = args.as_ref();
    let mut args = vec![std::ffi::OsString::from(cmd.get_name())];
    let arg_index;

    if let Some((prior, after)) = input.split_once("[TAB]") {
        args.extend(prior.split_whitespace().map(From::from));
        if prior.ends_with(char::is_whitespace) {
            args.push(std::ffi::OsString::default());
        }
        arg_index = args.len() - 1;
        // HACK: this cannot handle in-word '[TAB]'
        args.extend(after.split_whitespace().map(From::from));
    } else {
        args.extend(input.split_whitespace().map(From::from));
        if input.ends_with(char::is_whitespace) {
            args.push(std::ffi::OsString::default());
        }
        arg_index = args.len() - 1;
    }

    clap_complete::engine::complete(cmd, args, arg_index, current_dir)
        .unwrap()
        .into_iter()
        .map(|candidate| {
            let compl = candidate.get_value().to_str().unwrap();
            if let Some(help) = candidate.get_help() {
                format!("{compl}\t{help}")
            } else {
                compl.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
