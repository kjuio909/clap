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
fn deep_subcommands() {
    let name = "demo";
    let cmd = common::deep_subcommands_command(name);
    common::assert_matches(
        snapbox::file!["../snapshots/deep_subcommands.fish"],
        clap_complete::shells::Fish,
        cmd,
        name,
    );
}

/// End-to-end regression test for arbitrarily deep subcommands: source the generated fish script
/// and assert the candidate names reported by `complete -C`.  Two branches intentionally share
/// the `beta`/`gamma` names, so this also verifies that the full ordered command chain is matched
/// and the two branches never cross-wire their completions.
#[cfg(unix)]
#[test]
fn deep_subcommands_completions() {
    let Some(fish) = fish_binary() else {
        // fish is not installed; the static snapshot test still guards code generation.
        return;
    };

    let name = "demo";
    let mut cmd = common::deep_subcommands_command(name);
    let mut buf = vec![];
    clap_complete::generate(clap_complete::aot::Fish, &mut cmd, name, &mut buf);

    let script = std::env::temp_dir().join("clap_complete_deep_subcommands.fish");
    std::fs::write(&script, &buf).unwrap();

    // `(query, expected candidate names)` — each list must match exactly, in any order.
    let cases: &[(&str, &[&str])] = &[
        ("demo alpha beta gamma --m", &["--mode"]),
        ("demo alpha beta gamma --mode f", &["fast"]),
        ("demo alpha beta gamma --mode s", &["safe"]),
        ("demo alpha beta gamma d", &["delta"]),
        ("demo alpha beta gamma delta --l", &["--leaf"]),
        ("demo omega beta gamma --o", &["--other"]),
        // The omega branch has no --mode and no delta.
        ("demo omega beta gamma --m", &[]),
    ];
    for (query, expected) in cases {
        let mut actual = complete_candidates(&fish, &script, query);
        actual.sort();
        let mut expected: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
        expected.sort();
        assert_eq!(
            actual, expected,
            "candidate mismatch for `{query}`"
        );
    }

    // The other branch's subcommand must never leak through the shared beta/gamma names.
    assert!(
        !complete_candidates(&fish, &script, "demo omega beta gamma d").contains(&"delta".to_owned()),
        "omega branch leaked the alpha branch's `delta` subcommand"
    );
    assert!(
        !complete_candidates(&fish, &script, "demo alpha beta gamma --other")
            .contains(&"--other".to_owned()),
        "alpha branch leaked the omega branch's `--other` flag"
    );

    let _ = std::fs::remove_file(&script);
}

#[cfg(unix)]
fn fish_binary() -> Option<std::path::PathBuf> {
    let candidates = || std::env::var_os("PATH").map(|paths| {
        std::env::split_paths(&paths)
            .map(|p| p.join("fish"))
            .collect::<Vec<_>>()
    });
    let mut candidates = candidates().unwrap_or_default();
    // Common non-PATH install locations (e.g. Homebrew on macOS).
    candidates.push(std::path::PathBuf::from("/opt/homebrew/bin/fish"));
    candidates.push(std::path::PathBuf::from("/usr/local/bin/fish"));
    candidates
        .into_iter()
        .find(|p| p.is_file() && std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(p).unwrap().permissions()) & 0o111 != 0)
}

#[cfg(unix)]
fn complete_candidates(
    fish: &std::path::Path,
    script: &std::path::Path,
    query: &str,
) -> Vec<String> {
    let code = format!(
        "source '{script}'; complete -C '{query}'",
        script = script.display()
    );
    let output = std::process::Command::new(fish)
        .arg("-c")
        .arg(&code)
        .output()
        .expect("failed to run fish");
    assert!(
        output.status.success(),
        "fish failed for `{query}`:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        // `complete -C` prints `candidate\tdescription`; keep the name before the tab.
        .map(|line| line.split('\t').next().unwrap_or("").to_owned())
        .filter(|name| !name.is_empty())
        .collect()
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
    let expected = snapbox::str![[r#"
% exhaustive 
action  empty   help  (Print this message or the help of the given subcommand(s))  last    quote
alias   global  hint                                                               pacman  value
"#]];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive empty \t";
    let expected = snapbox::str![[r#"
% exhaustive empty 
Cargo.toml    CONTRIBUTING.md  LICENSE-APACHE  README.md  tests/
CHANGELOG.md  examples/        LICENSE-MIT     src/       
"#]];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive --empty=\t";
    let expected = snapbox::str!["% exhaustive --empty="];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive quote --choice \t";
    let actual = runtime.complete(input, &term).unwrap();
    let expected = snapbox::str![[r#"
% exhaustive quote --choice 
another  bash  (bash (shell))  fish  (fish shell)  shell  (something with a space)  zsh  (zsh shell)
"#]];
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
    let expected = snapbox::str![[r#"
% exhaustive empty 
empty   quote   last   help  (Print this message or the help of the given subcommand(s))  --help  (Print help)
global  value   alias  --generate                                             (generate)  
action  pacman  hint   --empty-choice                                                     
"#]];
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
    let expected = snapbox::str![[r#"
% exhaustive quote cmd-single-quotes 
cmd-single-quotes           (Can be 'always', 'auto', or 'never')
cmd-double-quotes           (Can be "always", "auto", or "never")
cmd-backticks              (For more information see `echo test`)
cmd-backslash                                        (Avoid '/n')
cmd-brackets                             (List packages [filter])
cmd-expansions            (Execute the shell command with $SHELL)
escape-help                                             (/tab "')
help  (Print this message or the help of the given subcommand(s))
--single-quotes             (Can be 'always', 'auto', or 'never')
--double-quotes             (Can be "always", "auto", or "never")
--backticks                (For more information see `echo test`)
--backslash                                          (Avoid '/n')
--brackets                               (List packages [filter])
--expansions              (Execute the shell command with $SHELL)
--choice                                                         
--help                      (Print help (see more with '--help'))
"#]];
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
    let expected = snapbox::str![[r#"
% exhaustive action --choice=first 
--choice=first  --choice=second
"#]];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive action --choice=f\t";
    let expected = snapbox::str!["% exhaustive action --choice=first "];
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
    let expected = snapbox::str![[r#"
% exhaustive quote --choice another/ shell 
another shell  (something with a space)  bash  (bash (shell))  fish  (fish shell)  zsh  (zsh shell)
"#]];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);

    let input = "exhaustive quote --choice an\t";
    let expected = snapbox::str!["% exhaustive quote --choice another/ shell "];
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
    let expected = snapbox::str!["% exhaustive empty "];
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
    let expected = snapbox::str!["% exhaustive --empty="];
    let actual = runtime.complete(input, &term).unwrap();
    assert_data_eq!(actual, expected);
}
