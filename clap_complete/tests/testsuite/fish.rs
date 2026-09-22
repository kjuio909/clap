use crate::common;
#[allow(unused_imports)]
use snapbox::assert_data_eq;

#[cfg(unix)]
#[cfg(feature = "unstable-shell-tests")]
const CMD: &str = "fish";
#[cfg(unix)]
#[cfg(feature = "unstable-shell-tests")]
type RuntimeBuilder = completest_pty::FishRuntimeBuilder;

#[test]
fn basic() {
    let name = "my-app";
    let cmd = common::basic_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/basic.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn feature_sample() {
    let name = "my-app";
    let cmd = common::feature_sample_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/feature_sample.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn special_commands() {
    let name = "my-app";
    let cmd = common::special_commands_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/special_commands.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn quoting() {
    let name = "my-app";
    let cmd = common::quoting_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/quoting.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn aliases() {
    let name = "my-app";
    let cmd = common::aliases_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/aliases.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn sub_subcommands() {
    let name = "my-app";
    let cmd = common::sub_subcommands_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/sub_subcommands.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn external_subcommands() {
    let name = "my-app";
    let cmd = common::external_subcommand(name);
    common::assert_matches(
        snapbox::file!["../snapshots/external_subcommands.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn custom_bin_name() {
    let name = "my-app";
    let bin_name = "bin-name";
    let cmd = common::basic_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/custom_bin_name.fish"],
        clap_complete::shells::Fish,
        cmd,
        bin_name,
    );
}

#[test]
fn value_hint() {
    let name = "my-app";
    let cmd = common::value_hint_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/value_hint.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn value_terminator() {
    let name = "my-app";
    let cmd = common::value_terminator_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/value_terminator.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn multi_value_option() {
    let name = "my-app";
    let cmd = common::multi_value_option_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/multi_value_option.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn optional_value_option() {
    let name = "my-app";
    let cmd = common::optional_value_option_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/optional_value_option.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn optional_multi_value_option() {
    let name = "my-app";
    let cmd = common::optional_multi_value_option_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/optional_multi_value_option.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn two_multi_valued_arguments() {
    let name = "my-app";
    let cmd = common::two_multi_valued_arguments_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/two_multi_valued_arguments.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

#[test]
fn subcommand_last() {
    let name = "my-app";
    let cmd = common::subcommand_last(name);
    common::assert_matches(
        snapbox::file!["../snapshots/subcommand_last.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

/// Two branches that share the subcommand names `beta`/`gamma` must not leak
/// completions into each other: the `-n` condition has to match the full,
/// ordered chain of subcommands, at any nesting depth.
#[test]
fn nested_subcommand_paths() {
    let name = "demo";
    let mut cmd = clap::Command::new(name)
        .subcommand(
            clap::Command::new("alpha").subcommand(clap::Command::new("beta").subcommand(
                clap::Command::new("gamma")
                    .arg(
                        clap::Arg::new("mode")
                            .long("mode")
                            .value_parser(["fast", "safe"]),
                    )
                    .subcommand(
                        clap::Command::new("delta").arg(
                            clap::Arg::new("leaf")
                                .long("leaf")
                                .action(clap::ArgAction::SetTrue),
                        ),
                    ),
            )),
        )
        .subcommand(
            clap::Command::new("omega").subcommand(clap::Command::new("beta").subcommand(
                clap::Command::new("gamma").arg(
                    clap::Arg::new("other")
                        .long("other")
                        .action(clap::ArgAction::SetTrue),
                ),
            )),
        );

    let mut buf = vec![];
    clap_complete::aot::generate(clap_complete::aot::Fish, &mut cmd, name, &mut buf);
    let completion = String::from_utf8(buf).unwrap();

    let Ok(fish) = find_fish() else {
        eprintln!("`fish` not found in PATH, skipping runtime checks");
        return;
    };

    // Complete in an empty directory so file completion cannot add candidates.
    let scratch = std::env::temp_dir().join(format!("clap-fish-test-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    let script = scratch.join("demo.fish");
    std::fs::write(&script, &completion).unwrap();

    let cases: &[(&str, &[&str])] = &[
        ("demo alpha beta gamma --m", &["--mode"]),
        ("demo alpha beta gamma --mode f", &["fast"]),
        ("demo alpha beta gamma d", &["delta"]),
        ("demo alpha beta gamma delta --l", &["--leaf"]),
        ("demo omega beta gamma --o", &["--other"]),
        ("demo omega beta gamma --m", &[]),
    ];
    for (line, expected) in cases {
        let output = std::process::Command::new(&fish)
            .arg("--no-config")
            .arg("--command")
            .arg(format!(
                "source {}; complete -C {}",
                script.display(),
                shell_escape(line)
            ))
            .current_dir(&scratch)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "fish failed for `{line}`: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let mut actual: Vec<&str> = stdout
            .lines()
            .map(|line| line.split('\t').next().unwrap())
            .collect();
        actual.sort_unstable();
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        assert_eq!(&actual, &expected, "unexpected candidates for `{line}`");
    }

    std::fs::remove_dir_all(&scratch).ok();
}

#[cfg(unix)]
fn find_fish() -> std::io::Result<std::path::PathBuf> {
    // `fish` is not always on PATH (e.g. Homebrew on some CI images)
    for candidate in ["fish".into(), "/opt/homebrew/bin/fish".into()] {
        if std::process::Command::new(&candidate)
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
        {
            return Ok(candidate);
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "`fish` not found",
    ))
}

#[cfg(not(unix))]
fn find_fish() -> std::io::Result<std::path::PathBuf> {
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "`fish` not supported on this platform",
    ))
}

fn shell_escape(arg: &str) -> String {
    format!("'{}'", arg.replace('\'', "\\'"))
}

#[test]
#[cfg(unix)]
#[cfg(feature = "unstable-shell-tests")]
fn register_completion() {
    common::register_example::<RuntimeBuilder>("static", "exhaustive");
}

#[test]
#[cfg(unix)]
#[cfg(feature = "unstable-shell-tests")]
fn complete() {
    if !common::has_command(CMD) {
        return;
    }

    let term = completest::Term::new();
    let mut runtime = common::load_runtime::<RuntimeBuilder>("static", "exhaustive");

    let input = "exhaustive \t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive empty \t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive --empty=\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive quote --choice \t";
    let actual = runtime.complete(input, &term).unwrap();
    let expected = snapbox::str![""];
    assert_data_eq!(actual, expected);
}

#[test]
#[cfg(all(unix, feature = "unstable-dynamic"))]
#[cfg(feature = "unstable-shell-tests")]
fn register_dynamic_env() {
    common::register_example::<RuntimeBuilder>("dynamic-env", "exhaustive");
}

#[test]
#[cfg(all(unix, feature = "unstable-dynamic"))]
#[cfg(feature = "unstable-shell-tests")]
fn complete_dynamic_env_toplevel() {
    if !common::has_command(CMD) {
        return;
    }

    let term = completest::Term::new();
    let mut runtime = common::load_runtime::<RuntimeBuilder>("dynamic-env", "exhaustive");

    let input = "exhaustive \t\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);
}

#[test]
#[cfg(all(unix, feature = "unstable-dynamic"))]
#[cfg(feature = "unstable-shell-tests")]
fn complete_dynamic_env_quoted_help() {
    if !common::has_command(CMD) {
        return;
    }

    let term = completest::Term::new();
    let mut runtime = common::load_runtime::<RuntimeBuilder>("dynamic-env", "exhaustive");

    let input = "exhaustive quote \t\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);
}

#[test]
#[cfg(all(unix, feature = "unstable-dynamic"))]
#[cfg(feature = "unstable-shell-tests")]
fn complete_dynamic_env_option_value() {
    if !common::has_command(CMD) {
        return;
    }

    let term = completest::Term::new();
    let mut runtime = common::load_runtime::<RuntimeBuilder>("dynamic-env", "exhaustive");

    let input = "exhaustive action --choice=\t\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive action --choice=f\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);
}

#[test]
#[cfg(all(unix, feature = "unstable-dynamic"))]
#[cfg(feature = "unstable-shell-tests")]
fn complete_dynamic_env_quoted_value() {
    if !common::has_command(CMD) {
        return;
    }

    let term = completest::Term::new();
    let mut runtime = common::load_runtime::<RuntimeBuilder>("dynamic-env", "exhaustive");

    let input = "exhaustive quote --choice \t\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive quote --choice an\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);
}

#[test]
#[cfg(all(unix, feature = "unstable-dynamic"))]
#[cfg(feature = "unstable-shell-tests")]
fn complete_dynamic_empty_subcommand() {
    if !common::has_command(CMD) {
        return;
    }

    let term = completest::Term::new();
    let mut runtime = common::load_runtime::<RuntimeBuilder>("dynamic-env", "exhaustive");

    let input = "exhaustive empty \t\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);
}

#[test]
#[cfg(all(unix, feature = "unstable-dynamic"))]
#[cfg(feature = "unstable-shell-tests")]
fn complete_dynamic_empty_option_value() {
    if !common::has_command(CMD) {
        return;
    }

    let term = completest::Term::new();
    let mut runtime = common::load_runtime::<RuntimeBuilder>("dynamic-env", "exhaustive");

    let input = "exhaustive --empty=\t";
    let expected = snapbox::str![""];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);
}
