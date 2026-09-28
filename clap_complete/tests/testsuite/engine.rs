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
--delimiter=comma,a_pos
--delimiter=comma,b_pos
--delimiter=comma,c_pos
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
-D=comma,a_pos
-D=comma,b_pos
-D=comma,c_pos
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

fn inspect_command() -> Command {
    fn name_candidates() -> Vec<CompletionCandidate> {
        vec![
            CompletionCandidate::new("alice"),
            CompletionCandidate::new("alina"),
            // Repeated on purpose: only the first occurrence may be completed
            CompletionCandidate::new("alina"),
            CompletionCandidate::new("bob"),
        ]
    }

    Command::new("inspect")
        .alias("i")
        .arg(
            clap::Arg::new("name")
                .long("name")
                .add(ArgValueCandidates::new(name_candidates)),
        )
        .arg(
            clap::Arg::new("format")
                .long("format")
                .value_parser(["json", "yaml"]),
        )
        .arg(clap::Arg::new("query").value_parser(["all", "changed"]))
        .arg(
            clap::Arg::new("internal")
                .long("internal")
                .action(clap::ArgAction::SetTrue)
                .hide(true),
        )
        .subcommand(
            Command::new("debug")
                .hide(true)
                .arg(clap::Arg::new("level").value_parser(["on", "off"])),
        )
}

/// The full command tree: direct applets `busybox`, `inspect` and `status`,
/// with `busybox` also dispatching to `inspect`.
fn custom_candidates_command() -> Command {
    Command::new("tool")
        .subcommand(Command::new("busybox").subcommand(inspect_command()))
        .subcommand(inspect_command())
        .subcommand(Command::new("status"))
}

/// Call `clap_complete::engine::complete` with a full argv (including argv0),
/// the cursor sitting on the last token, and return the candidate values.
fn complete_argv(cmd: &mut Command, argv: &[&str]) -> Vec<String> {
    let arg_index = argv.len() - 1;
    let args = argv.iter().map(std::ffi::OsString::from).collect();
    clap_complete::engine::complete(cmd, args, arg_index, None)
        .unwrap()
        .into_iter()
        .map(|candidate| candidate.get_value().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn suggest_custom_arg_value_candidates_via_alias() {
    let mut cmd = custom_candidates_command();

    let expected = vec!["alice", "alina", "bob"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "--name", ""]),
        expected
    );
    // The alias entry point must produce identical candidates, in order
    assert_eq!(complete_argv(&mut cmd, &["tool", "i", "--name", ""]), expected);

    let expected = vec!["alice", "alina"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "--name", "al"]),
        expected
    );
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "i", "--name", "al"]),
        expected
    );

    assert!(complete_argv(&mut cmd, &["tool", "inspect", "--name", "z"]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "i", "--name", "z"]).is_empty());

    // `--name=value` keeps the `=` and the typed prefix in the candidates
    let expected = vec!["--name=alice", "--name=alina"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "--name=al"]),
        expected
    );
    assert_eq!(complete_argv(&mut cmd, &["tool", "i", "--name=al"]), expected);

    // A repeated `--name` still resolves to the same provider, without
    // leaking `--format` or positional values
    let expected = vec!["alice", "alina", "bob"];
    assert_eq!(
        complete_argv(
            &mut cmd,
            &["tool", "inspect", "--name", "alice", "--name", ""]
        ),
        expected
    );
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "i", "--name", "alice", "--name", ""]),
        expected
    );

    // After `--`, only positional values are offered
    let expected = vec!["all", "changed"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "--", ""]),
        expected
    );
    assert_eq!(complete_argv(&mut cmd, &["tool", "i", "--", ""]), expected);

    // Static values are unaffected
    let expected = vec!["json", "yaml"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "--format", ""]),
        expected
    );
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "i", "--format", ""]),
        expected
    );

    // Unknown aliases, options, and prefixes return empty rather than
    // falling back to root command candidates
    assert!(complete_argv(&mut cmd, &["tool", "unknown", "--name", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "--name", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "inspect", "--unknown", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "inspect", "--name=z"]).is_empty());
}

#[test]
fn suggest_custom_arg_value_candidates_multicall() {
    // Each suffix, appended to an entry point, forms a full argv whose last
    // token holds the cursor.
    let suffixes: &[&[&str]] = &[
        &["--name", ""],
        &["--name", "al"],
        &["--name", "z"],
        &["--name=al"],
        &["--name", "alice", "--name", ""],
        &["--format", ""],
        &["--", ""],
        // A fully typed hidden token stays parseable at every entry point
        &["--internal", "--format", ""],
        &["debug", "--", ""],
    ];
    // Every recognized entry point must resolve to the same `inspect` command
    // state as the ordinary `tool inspect` invocation.
    let entry_points: &[&[&str]] = &[
        &["/usr/bin/inspect"],
        &["/opt/tools/i.exe"],
        &["/usr/bin/i"],
        &["/usr/bin/busybox", "inspect"],
        &["/usr/bin/busybox", "i"],
    ];

    for suffix in suffixes {
        let mut reference_argv = vec!["tool", "inspect"];
        reference_argv.extend_from_slice(suffix);
        let mut cmd = custom_candidates_command();
        let expected = complete_argv(&mut cmd, &reference_argv);

        for entry_point in entry_points {
            let mut argv = entry_point.to_vec();
            argv.extend_from_slice(suffix);
            let mut cmd = custom_candidates_command().multicall(true);
            let actual = complete_argv(&mut cmd, &argv);
            assert_eq!(actual, expected, "argv={argv:?}");
        }
    }
}

#[test]
fn suggest_custom_arg_value_candidates_multicall_unknown() {
    // Recognized direct applets complete against their own command
    let mut cmd = custom_candidates_command().multicall(true);
    assert_eq!(
        complete_argv(&mut cmd, &["/usr/bin/status", ""]),
        vec!["--help"]
    );
    let mut cmd = custom_candidates_command().multicall(true);
    assert_eq!(
        complete_argv(&mut cmd, &["/usr/bin/busybox", ""]),
        vec!["inspect", "help", "--help"]
    );

    // Unknown invocation names must not fall back to root command candidates
    let mut cmd = custom_candidates_command().multicall(true);
    assert!(complete_argv(&mut cmd, &["/usr/bin/unknown", "--name", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["/usr/bin/unknown", "--name", "al"]).is_empty());
    assert!(complete_argv(&mut cmd, &["/opt/bin/unknown.exe", "--name=al"]).is_empty());
    assert!(complete_argv(&mut cmd, &["/usr/bin/unknown", "zzz", ""]).is_empty());
    // A partial applet spelling is not an entry point
    assert!(complete_argv(&mut cmd, &["/usr/bin/ins", "--name", ""]).is_empty());

    // Unknown busybox sub-applets never fall back to another applet or root
    assert!(complete_argv(&mut cmd, &["/usr/bin/busybox", "unknown", "--name", ""]).is_empty());
    // `status` is a direct applet, not a sub-applet of `busybox`
    assert!(complete_argv(&mut cmd, &["/usr/bin/busybox", "status", ""]).is_empty());

    // Unknown options and illegal prefixes stay empty through every entry point
    assert!(complete_argv(&mut cmd, &["/usr/bin/inspect", "--name", "z"]).is_empty());
    assert!(complete_argv(&mut cmd, &["/opt/tools/i.exe", "--name", "z"]).is_empty());
    assert!(complete_argv(&mut cmd, &["/usr/bin/inspect", "--unknown", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["/usr/bin/busybox", "inspect", "--name", "z"]).is_empty());
}

#[test]
fn suggest_hidden_items_custom_values_deduped() {
    let mut cmd = custom_candidates_command();

    // Duplicates collapse to their first occurrence, declaration order is kept
    let expected = vec!["alice", "alina", "bob"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "--name", ""]),
        expected
    );
    assert_eq!(complete_argv(&mut cmd, &["tool", "i", "--name", ""]), expected);

    let expected = vec!["alice", "alina"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "--name", "al"]),
        expected
    );
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "i", "--name", "al"]),
        expected
    );
}

#[test]
fn suggest_hidden_items_not_offered_on_empty_prefix() {
    let mut cmd = custom_candidates_command();

    // Only public options and public paths: no `--internal`, no `debug`
    let expected = vec![
        "help", "all", "changed", "--name", "--format", "--help",
    ];
    assert_eq!(complete_argv(&mut cmd, &["tool", "inspect", ""]), expected);
    assert_eq!(complete_argv(&mut cmd, &["tool", "i", ""]), expected);
}

#[test]
fn suggest_hidden_subcommand_values_after_escape() {
    let mut cmd = custom_candidates_command();

    let expected = vec!["on", "off"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "debug", "--", ""]),
        expected
    );
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "i", "debug", "--", ""]),
        expected
    );
}

#[test]
fn suggest_hidden_items_after_fully_typed_hidden_flag() {
    let mut cmd = custom_candidates_command();

    let expected = vec!["json", "yaml"];
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "inspect", "--internal", "--format", ""]),
        expected
    );
    assert_eq!(
        complete_argv(&mut cmd, &["tool", "i", "--internal", "--format", ""]),
        expected
    );
}

#[test]
fn suggest_hidden_items_unknown_paths_empty() {
    let mut cmd = custom_candidates_command();

    // Unknown command, unknown alias, and partial names never fall back to
    // root command candidates
    assert!(complete_argv(&mut cmd, &["tool", "unknown", "--name", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "ii", "--name", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "ins", "--name", ""]).is_empty());

    // Illegal candidate prefixes stay empty
    assert!(complete_argv(&mut cmd, &["tool", "inspect", "--name", "z"]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "i", "--name", "z"]).is_empty());

    // Incomplete spellings of hidden tokens stay empty; in particular a
    // partial hidden subcommand followed by `--` must not surface the public
    // `help` subcommand or positional values.
    assert!(complete_argv(&mut cmd, &["tool", "inspect", "--int", "--format", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "inspect", "deb", "--", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "i", "--int", "--format", ""]).is_empty());
    assert!(complete_argv(&mut cmd, &["tool", "i", "deb", "--", ""]).is_empty());
}

/// Root command used by the interleaved optional/required value tests:
/// - `--color` is a repeatable option whose value is *optional*
///   (`auto`/`always`/`never`),
/// - `--format` takes one required value (`json`/`yaml`) and cannot repeat,
/// - the repeatable `files` positional always offers `a.txt`/`b.txt`.
fn interleaved_command() -> Command {
    Command::new("tool")
        .arg(
            clap::Arg::new("color")
                .long("color")
                .action(clap::ArgAction::Append)
                .num_args(0..=1)
                .value_parser(["auto", "always", "never"]),
        )
        .arg(
            clap::Arg::new("format")
                .long("format")
                .value_parser(["json", "yaml"]),
        )
        .arg(
            clap::Arg::new("files")
                .action(clap::ArgAction::Append)
                .num_args(1..)
                .value_parser(["a.txt", "b.txt"]),
        )
}

/// Strip an attached `--flag=` / `-f=` prefix so the space-attached and
/// equals-attached forms can be compared by their underlying values.
fn strip_attached_prefix(prefix: &str, values: Vec<String>) -> Vec<String> {
    values
        .into_iter()
        .map(|value| match value.strip_prefix(prefix) {
            Some(stripped) => stripped.to_owned(),
            None => value,
        })
        .collect()
}

#[test]
fn interleaved_empty_value_positions() {
    // The optional `--color` offers every declared value at its empty value
    // position, with no positional or flag candidates leaking in.
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--color", ""]),
        vec!["auto", "always", "never"]
    );
    // The equals form carries the `--color=` replacement prefix but lists the
    // same values in the same order.
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--color="]),
        vec![
            "--color=auto",
            "--color=always",
            "--color=never"
        ]
    );

    // The required-value `--format` offers exactly its two values.
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--format", ""]),
        vec!["json", "yaml"]
    );
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--format="]),
        vec!["--format=json", "--format=yaml"]
    );
}

#[test]
fn interleaved_prefix_filtering() {
    // A partial value only narrows the current option; it never changes other
    // tokens' interpretation.
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--color", "au"]),
        vec!["auto"]
    );
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--color=au"]),
        vec!["--color=auto"]
    );
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--format", "ya"]),
        vec!["yaml"]
    );
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--format=ya"]),
        vec!["--format=yaml"]
    );

    // Space and equals forms agree on the underlying values at the same cursor.
    let spaced = complete_argv(&mut interleaved_command(), &["tool", "--color", "au"]);
    let attached = strip_attached_prefix(
        "--color=",
        complete_argv(&mut interleaved_command(), &["tool", "--color=au"]),
    );
    assert_eq!(spaced, attached);

    // Non-matching prefixes yield nothing rather than other options' values.
    assert!(complete_argv(&mut interleaved_command(), &["tool", "--color", "z"]).is_empty());
    assert!(complete_argv(&mut interleaved_command(), &["tool", "--format=x"]).is_empty());
}

#[test]
fn interleaved_optional_value_omitted_before_required() {
    // `--color` followed by another flag omits the optional value; the cursor
    // serves `--format` only, so no color values are mixed in.
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--color", "--format", ""]),
        vec!["json", "yaml"]
    );
    assert_eq!(
        complete_argv(
            &mut interleaved_command(),
            &["tool", "--color", "--format", "js"]
        ),
        vec!["json"]
    );

    // An attached empty color value is invalid, so the incomplete `js` value of
    // `--format` cannot surface format candidates and color values never leak.
    assert!(
        complete_argv(&mut interleaved_command(), &["tool", "--color=", "--format", "js"])
            .is_empty()
    );

    // A committed, non-cursor `au` token is an invalid color value; completion
    // stops instead of serving `--format`.
    assert!(
        complete_argv(&mut interleaved_command(), &["tool", "--color", "au", "--format", ""])
            .is_empty()
    );

    // The required-value option cannot omit its value for `--color`.
    assert!(
        complete_argv(&mut interleaved_command(), &["tool", "--format", "--color", ""]).is_empty()
    );
    assert!(
        complete_argv(&mut interleaved_command(), &["tool", "--format", "--color"]).is_empty()
    );
}

#[test]
fn interleaved_repeated_optional_keeps_full_set() {
    let colors = vec!["auto", "always", "never"];
    let attached_colors = vec![
        "--color=auto",
        "--color=always",
        "--color=never",
    ];

    // Each independent occurrence offers the full set in declaration order,
    // regardless of previously completed prefixes.
    assert_eq!(
        complete_argv(
            &mut interleaved_command(),
            &["tool", "--color", "always", "--color", ""]
        ),
        colors
    );
    assert_eq!(
        complete_argv(
            &mut interleaved_command(),
            &["tool", "--color=always", "--color="]
        ),
        attached_colors
    );
    assert_eq!(
        complete_argv(
            &mut interleaved_command(),
            &["tool", "--color", "always", "--color", "always", "--color", ""]
        ),
        colors
    );

    // Partial values within later occurrences stay focused on that occurrence.
    assert_eq!(
        complete_argv(
            &mut interleaved_command(),
            &["tool", "--color", "always", "--color", "nev"]
        ),
        vec!["never"]
    );
    assert_eq!(
        complete_argv(
            &mut interleaved_command(),
            &["tool", "--color=always", "--color=au"]
        ),
        vec!["--color=auto"]
    );

    // The two spellings produce the same underlying text sequence after every
    // completed occurrence.
    let spaced = complete_argv(
        &mut interleaved_command(),
        &["tool", "--color", "always", "--color", ""],
    );
    let attached = strip_attached_prefix(
        "--color=",
        complete_argv(
            &mut interleaved_command(),
            &["tool", "--color=always", "--color="],
        ),
    );
    assert_eq!(spaced, attached);
    assert_eq!(spaced, colors);
}

#[test]
fn interleaved_repeated_required_is_empty() {
    // `--format` cannot be repeated, so actively supplying another value gives
    // no candidates in either spelling.
    assert!(
        complete_argv(&mut interleaved_command(), &["tool", "--format", "json", "--format", ""])
            .is_empty()
    );
    assert!(
        complete_argv(&mut interleaved_command(), &["tool", "--format", "json", "--format=j"])
            .is_empty()
    );

    // But the flag name itself is still listed in the option menu, and an
    // already satisfied value leaves normal completion available.
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--format", "json", ""]),
        vec!["a.txt", "b.txt", "--color", "--format", "--help"]
    );
    assert_eq!(
        complete_argv(
            &mut interleaved_command(),
            &["tool", "--format", "json", "--format"]
        ),
        vec!["--format"]
    );
}

#[test]
fn interleaved_after_escape_only_positionals() {
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--", ""]),
        vec!["a.txt", "b.txt"]
    );
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--", "a"]),
        vec!["a.txt"]
    );

    // Tokens that look like options are treated purely as positional values.
    assert!(complete_argv(&mut interleaved_command(), &["tool", "--", "--color"]).is_empty());
    assert!(complete_argv(&mut interleaved_command(), &["tool", "--", "--format"]).is_empty());

    // The positional keeps offering its fixed candidates after a value.
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "--", "a.txt", ""]),
        vec!["a.txt", "b.txt"]
    );
    assert_eq!(
        complete_argv(&mut interleaved_command(), &["tool", "a.txt", ""]),
        vec!["a.txt", "b.txt", "--color", "--format", "--help"]
    );
}

#[test]
fn interleaved_unknown_and_invalid_stay_empty() {
    // Unknown options never fall back to root-command candidates.
    assert!(complete_argv(&mut interleaved_command(), &["tool", "--unknown", ""]).is_empty());
    assert!(complete_argv(&mut interleaved_command(), &["tool", "--unknown=x"]).is_empty());

    // Invalid committed values invalidate the whole line.
    assert!(complete_argv(&mut interleaved_command(), &["tool", "--color=xx"]).is_empty());
    assert!(complete_argv(&mut interleaved_command(), &["tool", "--color", "xx"]).is_empty());
    assert!(
        complete_argv(&mut interleaved_command(), &["tool", "--color=xx", "--format", ""])
            .is_empty()
    );
    assert!(
        complete_argv(&mut interleaved_command(), &["tool", "--format", "xml", ""]).is_empty()
    );
}

#[test]
fn interleaved_state_is_rebuilt_per_call() {
    let mut cmd = interleaved_command();

    let run = |cmd: &mut Command, argv: &[&str]| -> Vec<String> {
        let arg_index = argv.len() - 1;
        let args = argv.iter().map(std::ffi::OsString::from).collect();
        clap_complete::engine::complete(cmd, args, arg_index, None)
            .unwrap()
            .into_iter()
            .map(|c| c.get_value().to_string_lossy().into_owned())
            .collect()
    };

    let colors = vec!["auto", "always", "never"];

    // A successful call on the reused command does not influence the next one.
    assert_eq!(run(&mut cmd, &["tool", "--format", "json", "--color", ""]), colors);
    assert_eq!(run(&mut cmd, &["tool", "--color", ""]), colors);

    // Neither does a failed call leak its partial match or error state.
    assert!(run(&mut cmd, &["tool", "--color=zz", "--format", ""]).is_empty());
    assert_eq!(run(&mut cmd, &["tool", "--color", ""]), colors);
    assert_eq!(run(&mut cmd, &["tool", "--format", ""]), vec!["json", "yaml"]);
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
