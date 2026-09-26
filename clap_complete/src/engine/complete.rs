use std::collections::HashSet;
use std::ffi::OsStr;
use std::ffi::OsString;

use clap::Id;
use clap_lex::OsStrExt as _;

use super::ArgValueCandidates;
use super::ArgValueCompleter;
use super::CompletionCandidate;
use super::SubcommandCandidates;
use super::ValueCandidates;
use super::custom::complete_path;
use super::custom::possible_value_candidates;

/// Complete the given command, shell-agnostic
pub fn complete(
    cmd: &mut clap::Command,
    args: Vec<OsString>,
    arg_index: usize,
    current_dir: Option<&std::path::Path>,
) -> Result<Vec<CompletionCandidate>, std::io::Error> {
    debug!("complete: args={args:?}, arg_index={arg_index:?}, current_dir={current_dir:?}");
    cmd.build();

    let raw_args = clap_lex::RawArgs::new(args);
    let mut cursor = raw_args.cursor();
    let mut target_cursor = raw_args.cursor();
    raw_args.seek(
        &mut target_cursor,
        clap_lex::SeekFrom::Start(arg_index as u64),
    );
    // As we loop, `cursor` will always be pointing to the next item
    raw_args.next_os(&mut target_cursor);
    debug!("complete: target_cursor={target_cursor:?}");

    // TODO: Multicall support
    if !cmd.is_no_binary_name_set() {
        raw_args.next_os(&mut cursor);
    }

    let mut current_cmd = &*cmd;
    let mut pos_index = 1;
    let mut is_escaped = false;
    let mut next_state = ParseState::ValueDone;
    // Ids of options explicitly present in `args[..arg_index]`, used to hide
    // conflicting candidates. Only recognized option names are recorded;
    // values, positionals, `--`-escaped words, default values and env values
    // never are.
    let mut explicit_opts = HashSet::<Id>::new();
    while let Some(arg) = raw_args.next(&mut cursor) {
        let current_state = next_state;
        next_state = ParseState::ValueDone;
        debug!(
            "complete::next: arg={:?}, current_state={current_state:?}, cursor={cursor:?}",
            arg.to_value_os(),
        );
        if cursor == target_cursor {
            let disabled = gather_disabled_args(current_cmd, &explicit_opts);
            return complete_arg(
                &arg,
                current_cmd,
                current_dir,
                pos_index,
                is_escaped,
                current_state,
                &disabled,
            );
        }

        if let Ok(value) = arg.to_value() {
            if let Some(next_cmd) = current_cmd.find_subcommand(value) {
                current_cmd = next_cmd;
                pos_index = 1;
                continue;
            }
        }

        if is_escaped {
            (next_state, pos_index) =
                parse_positional(current_cmd, pos_index, is_escaped, current_state);
        } else if arg.is_escape() {
            is_escaped = true;
        } else if opt_allows_hyphen(&current_state, &arg) {
            match current_state {
                ParseState::Opt((opt, count)) => next_state = parse_opt_value(opt, count),
                _ => unreachable!("else branch is only reachable in Opt state"),
            }
        } else if let Some((flag, value)) = arg.to_long() {
            if let Ok(flag) = flag {
                let opt = current_cmd.get_arguments().find(|a| {
                    let longs = a.get_long_and_visible_aliases();
                    let is_find = longs.map(|v| {
                        let mut iter = v.into_iter();
                        let s = iter.find(|s| *s == flag);
                        s.is_some()
                    });
                    is_find.unwrap_or(false)
                });

                if let Some(opt) = opt {
                    explicit_opts.insert(opt.get_id().clone());
                    if opt.get_num_args().expect("built").takes_values() && value.is_none() {
                        next_state = ParseState::Opt((opt, 1));
                    };
                } else if pos_allows_hyphen(current_cmd, pos_index) {
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, current_state);
                }
            }
        } else if let Some(short) = arg.to_short() {
            let (leading_flags, takes_value_opt, mut short, cluster_valid) =
                parse_shortflags(current_cmd, short);
            // Only a cluster made entirely of recognized flags records state.
            // Splitting an invalid short string into known and unknown members
            // would be guesswork and could suppress conflicting candidates, so
            // invalid clusters keep the existing completion behavior; clap's
            // parser would reject them anyway.
            if cluster_valid {
                // Every recognized flag in the cluster was explicitly supplied.
                for flag in leading_flags.chars() {
                    if let Some(opt) = current_cmd.get_arguments().find(|a| {
                        a.get_short_and_visible_aliases()
                            .is_some_and(|shorts| shorts.contains(&flag))
                    }) {
                        explicit_opts.insert(opt.get_id().clone());
                    }
                }
            }
            if let Some(opt) = takes_value_opt {
                if short.next_value_os().is_none() {
                    next_state = ParseState::Opt((opt, 1));
                }
            } else if pos_allows_hyphen(current_cmd, pos_index) {
                (next_state, pos_index) =
                    parse_positional(current_cmd, pos_index, is_escaped, current_state);
            }
        } else {
            match current_state {
                ParseState::ValueDone | ParseState::Pos(..) => {
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, current_state);
                }
                ParseState::Opt((opt, count)) => next_state = parse_opt_value(opt, count),
            }
        }
    }

    Err(std::io::Error::other("no completion generated"))
}

#[derive(Debug, PartialEq, Eq, Clone)]
enum ParseState<'a> {
    /// Parsing a value done, there is no state to record.
    ValueDone,

    /// Parsing a positional argument after `--`. `Pos(pos_index`, `takes_num_args`)
    Pos((usize, usize)),

    /// Parsing a optional flag argument
    Opt((&'a clap::Arg, usize)),
}

fn complete_arg(
    arg: &clap_lex::ParsedArg<'_>,
    cmd: &clap::Command,
    current_dir: Option<&std::path::Path>,
    pos_index: usize,
    is_escaped: bool,
    state: ParseState<'_>,
    disabled: &HashSet<Id>,
) -> Result<Vec<CompletionCandidate>, std::io::Error> {
    debug!(
        "complete_arg: arg={:?}, cmd={:?}, current_dir={:?}, pos_index={:?}, state={:?}",
        arg,
        cmd.get_name(),
        current_dir,
        pos_index,
        state
    );
    let mut completions = Vec::<CompletionCandidate>::new();

    match state {
        ParseState::ValueDone => {
            // After `--`, only positional arguments are completed.
            if !is_escaped {
                if let Ok(value) = arg.to_value() {
                    completions.extend(complete_subcommand(value, cmd));
                }
            }

            if let Some(positional) = cmd
                .get_positionals()
                .find(|p| p.get_index() == Some(pos_index))
            {
                if !disabled.contains(positional.get_id()) {
                    completions.extend(complete_arg_value(
                        arg.to_value(),
                        positional,
                        current_dir,
                        0,
                    ));
                }
            }
            if !is_escaped {
                completions.extend(complete_option(arg, cmd, current_dir, disabled));
            }
        }
        ParseState::Pos((_, num_arg)) => {
            if let Some(positional) = cmd
                .get_positionals()
                .find(|p| p.get_index() == Some(pos_index))
            {
                if !disabled.contains(positional.get_id()) {
                    completions.extend(complete_arg_value(
                        arg.to_value(),
                        positional,
                        current_dir,
                        num_arg.saturating_sub(1),
                    ));
                }
                if !is_escaped
                    && positional
                        .get_num_args()
                        .is_some_and(|num_args| num_arg >= num_args.min_values())
                {
                    completions.extend(complete_option(arg, cmd, current_dir, disabled));
                }
            }
        }
        ParseState::Opt((opt, count)) => {
            if !disabled.contains(opt.get_id()) {
                completions.extend(complete_arg_value(
                    arg.to_value(),
                    opt,
                    current_dir,
                    count.saturating_sub(1),
                ));
            }
            let min = opt.get_num_args().map(|r| r.min_values()).unwrap_or(0);
            if count > min {
                // Also complete this raw_arg as a positional argument, flags, options and subcommand.
                completions.extend(complete_arg(
                    arg,
                    cmd,
                    current_dir,
                    pos_index,
                    is_escaped,
                    ParseState::ValueDone,
                    disabled,
                )?);
            }
        }
    }
    if completions.iter().any(|a| !a.is_hide_set()) {
        completions.retain(|a| !a.is_hide_set());
    }
    let mut seen_ids = HashSet::new();
    completions.retain(move |a| {
        if let Some(id) = a.get_id().cloned() {
            seen_ids.insert(id)
        } else {
            true
        }
    });

    let mut tags = Vec::new();
    for candidate in &completions {
        let tag = candidate.get_tag().cloned();
        if !tags.contains(&tag) {
            tags.push(tag);
        }
    }
    completions.sort_by_key(|c| {
        (
            tags.iter().position(|t| c.get_tag() == t.as_ref()),
            c.get_display_order(),
        )
    });

    Ok(completions)
}

fn complete_option(
    arg: &clap_lex::ParsedArg<'_>,
    cmd: &clap::Command,
    current_dir: Option<&std::path::Path>,
    disabled: &HashSet<Id>,
) -> Vec<CompletionCandidate> {
    debug!("complete_option: arg={arg:?}, current_dir={current_dir:?}");
    let mut completions = Vec::<CompletionCandidate>::new();
    if arg.is_empty() {
        completions.extend(longs_and_visible_aliases(cmd, disabled));
        completions.extend(hidden_longs_aliases(cmd, disabled));

        let dash_or_arg = if arg.is_empty() {
            "-".into()
        } else {
            arg.to_value_os().to_string_lossy()
        };
        completions.extend(
            shorts_and_visible_aliases(cmd, disabled)
                .into_iter()
                .map(|comp| comp.add_prefix(dash_or_arg.to_string())),
        );
    } else if arg.is_stdio() {
        // HACK: Assuming knowledge of is_stdio
        let dash_or_arg = if arg.is_empty() {
            "-".into()
        } else {
            arg.to_value_os().to_string_lossy()
        };
        completions.extend(
            shorts_and_visible_aliases(cmd, disabled)
                .into_iter()
                .map(|comp| comp.add_prefix(dash_or_arg.to_string())),
        );

        completions.extend(longs_and_visible_aliases(cmd, disabled));
        completions.extend(hidden_longs_aliases(cmd, disabled));
    } else if arg.is_escape() {
        // HACK: Assuming knowledge of is_escape
        completions.extend(longs_and_visible_aliases(cmd, disabled));
        completions.extend(hidden_longs_aliases(cmd, disabled));
    } else if let Some((flag, value)) = arg.to_long() {
        if let Ok(flag) = flag {
            if let Some(value) = value {
                let opt = cmd.get_arguments().find(|a| {
                    a.get_long_and_visible_aliases()
                        .is_some_and(|longs| longs.into_iter().any(|long| long == flag))
                });
                if let Some(arg) = opt {
                    if !disabled.contains(arg.get_id()) {
                        completions.extend(
                            complete_arg_value(value.to_str().ok_or(value), arg, current_dir, 0)
                                .into_iter()
                                .map(|comp| comp.add_prefix(format!("--{flag}="))),
                        );
                    }
                }
            } else {
                completions.extend(
                    longs_and_visible_aliases(cmd, disabled)
                        .into_iter()
                        .filter(|comp| comp.get_value().starts_with(format!("--{flag}").as_str())),
                );
                completions.extend(
                    hidden_longs_aliases(cmd, disabled)
                        .into_iter()
                        .filter(|comp| comp.get_value().starts_with(format!("--{flag}").as_str())),
                );
            }
        }
    } else if let Some(short) = arg.to_short() {
        if !short.is_negative_number() {
            // Find the first takes_values option.
            let (leading_flags, takes_value_opt, mut short, _) = parse_shortflags(cmd, short);

            // Clone `short` to `peek_short` to peek whether the next flag is a `=`.
            if let Some(opt) = takes_value_opt {
                if !disabled.contains(opt.get_id()) {
                    let mut peek_short = short.clone();
                    let has_equal = if let Some(Ok('=')) = peek_short.next_flag() {
                        short.next_flag();
                        true
                    } else {
                        false
                    };

                    let value = short.next_value_os().unwrap_or(OsStr::new(""));
                    completions.extend(
                        complete_arg_value(value.to_str().ok_or(value), opt, current_dir, 0)
                            .into_iter()
                            .map(|comp| {
                                let sep = if has_equal { "=" } else { "" };
                                comp.add_prefix(format!("-{leading_flags}{sep}"))
                            }),
                    );
                }
            } else {
                completions.extend(
                    shorts_and_visible_aliases(cmd, disabled)
                        .into_iter()
                        .map(|comp| comp.add_prefix(format!("-{leading_flags}"))),
                );
            }
        }
    }
    debug!("complete_option: completions={completions:?}");
    completions
}

fn complete_arg_value(
    value: Result<&str, &OsStr>,
    arg: &clap::Arg,
    current_dir: Option<&std::path::Path>,
    arg_index: usize,
) -> Vec<CompletionCandidate> {
    let mut values = Vec::new();
    debug!("complete_arg_value: arg={arg:?}, value={value:?}, arg_index={arg_index:?}");

    let (prefix, value) =
        rsplit_delimiter(value, arg.get_value_delimiter()).unwrap_or((None, value));

    let value_os = match value {
        Ok(value) => OsStr::new(value),
        Err(value_os) => value_os,
    };

    if let Some(completer) = arg.get::<ArgValueCompleter>() {
        values.extend(completer.complete_at(arg_index, value_os));
    } else if let Some(completer) = arg.get::<ArgValueCandidates>() {
        values.extend(complete_value_candidates(
            value_os,
            completer.value_candidates(),
        ));
    } else if let Some(possible_values) = possible_values(arg) {
        if let Ok(value) = value {
            values.extend(complete_candidates_str(
                value,
                possible_value_candidates(possible_values),
            ));
        }
    } else {
        match arg.get_value_hint() {
            clap::ValueHint::Unknown | clap::ValueHint::Other => {
                // Should not complete
            }
            clap::ValueHint::AnyPath => {
                values.extend(complete_path(value_os, current_dir, &|_| true));
            }
            clap::ValueHint::FilePath => {
                values.extend(complete_path(value_os, current_dir, &|p| p.is_file()));
            }
            clap::ValueHint::DirPath => {
                values.extend(complete_path(value_os, current_dir, &|p| p.is_dir()));
            }
            clap::ValueHint::ExecutablePath => {
                use is_executable::IsExecutable;
                values.extend(complete_path(value_os, current_dir, &|p| p.is_executable()));
            }
            clap::ValueHint::CommandName
            | clap::ValueHint::CommandString
            | clap::ValueHint::CommandWithArguments
            | clap::ValueHint::Username
            | clap::ValueHint::Hostname
            | clap::ValueHint::Url
            | clap::ValueHint::EmailAddress => {
                // No completion implementation
            }
            _ => {
                // Safe-ish fallback
                values.extend(complete_path(value_os, current_dir, &|_| true));
            }
        }

        values.sort();
    }

    if let Some(prefix) = prefix {
        values = values
            .into_iter()
            .map(|comp| comp.add_prefix(prefix))
            .collect();
    }
    values = values
        .into_iter()
        .map(|comp| {
            if comp.get_tag().is_some() {
                comp
            } else {
                comp.tag(Some(arg.to_string().into()))
            }
        })
        .collect();

    debug!("complete_arg_value: values={values:?}");
    values
}

fn rsplit_delimiter<'s, 'o>(
    value: Result<&'s str, &'o OsStr>,
    delimiter: Option<char>,
) -> Option<(Option<&'s str>, Result<&'s str, &'o OsStr>)> {
    let delimiter = delimiter?;
    let value = value.ok()?;
    let pos = value.rfind(delimiter)?;
    let (prefix, value) = value.split_at(pos + delimiter.len_utf8());
    Some((Some(prefix), Ok(value)))
}

fn complete_value_candidates(
    value: &OsStr,
    completer: &dyn ValueCandidates,
) -> Vec<CompletionCandidate> {
    debug!("complete_value_candidates: value={value:?}");

    let mut values = completer.candidates();
    values.retain(|comp| comp.get_value().starts_with(&value.to_string_lossy()));
    values
}

fn complete_candidates_str(
    value: &str,
    mut values: Vec<CompletionCandidate>,
) -> Vec<CompletionCandidate> {
    debug!("complete_candidates_str: value={value:?}");

    values.retain(|comp| comp.get_value().starts_with(value));
    values
}

fn complete_value_candidates_str(
    value: &str,
    completer: &dyn ValueCandidates,
) -> Vec<CompletionCandidate> {
    debug!("complete_value_candidates_str: value={value:?}");

    complete_candidates_str(value, completer.candidates())
}

fn complete_subcommand(value: &str, cmd: &clap::Command) -> Vec<CompletionCandidate> {
    debug!(
        "complete_subcommand: cmd={:?}, value={:?}",
        cmd.get_name(),
        value
    );

    let mut scs: Vec<CompletionCandidate> = subcommands(cmd)
        .into_iter()
        .filter(|x| x.get_value().starts_with(value))
        .collect();
    if cmd.is_allow_external_subcommands_set() {
        let external_completer = cmd.get::<SubcommandCandidates>();
        if let Some(completer) = external_completer {
            scs.extend(complete_value_candidates_str(
                value,
                completer.value_candidates(),
            ));
        }
    }

    scs.sort();
    scs.dedup();
    scs
}

/// Gets all the long options, their visible aliases and flags of a [`clap::Command`] with formatted `--` prefix.
/// Includes `help` and `version` depending on the [`clap::Command`] settings.
fn longs_and_visible_aliases(
    p: &clap::Command,
    disabled: &HashSet<Id>,
) -> Vec<CompletionCandidate> {
    debug!("longs: name={}", p.get_name());

    p.get_arguments()
        .filter(|a| !disabled.contains(a.get_id()))
        .filter_map(|a| {
            a.get_long_and_visible_aliases().map(|longs| {
                longs
                    .into_iter()
                    .map(|s| populate_arg_candidate(CompletionCandidate::new(format!("--{s}")), a))
            })
        })
        .flatten()
        .collect()
}

/// Gets all the long hidden aliases and flags of a [`clap::Command`].
fn hidden_longs_aliases(p: &clap::Command, disabled: &HashSet<Id>) -> Vec<CompletionCandidate> {
    debug!("longs: name={}", p.get_name());

    p.get_arguments()
        .filter(|a| !disabled.contains(a.get_id()))
        .filter_map(|a| {
            a.get_aliases().map(|longs| {
                longs.into_iter().map(|s| {
                    populate_arg_candidate(CompletionCandidate::new(format!("--{s}")), a).hide(true)
                })
            })
        })
        .flatten()
        .collect()
}

/// Gets all the short options, their visible aliases and flags of a [`clap::Command`].
/// Includes `h` and `V` depending on the [`clap::Command`] settings.
fn shorts_and_visible_aliases(
    p: &clap::Command,
    disabled: &HashSet<Id>,
) -> Vec<CompletionCandidate> {
    debug!("shorts: name={}", p.get_name());

    p.get_arguments()
        .filter(|a| !disabled.contains(a.get_id()))
        .filter_map(|a| {
            a.get_short_and_visible_aliases().map(|shorts| {
                shorts.into_iter().map(|s| {
                    populate_arg_candidate(CompletionCandidate::new(s.to_string()), a).help(
                        a.get_help()
                            .cloned()
                            .or_else(|| a.get_long().map(|long| format!("--{long}").into())),
                    )
                })
            })
        })
        .flatten()
        .collect()
}

fn populate_arg_candidate(candidate: CompletionCandidate, arg: &clap::Arg) -> CompletionCandidate {
    candidate
        .help(arg.get_help().cloned())
        .id(Some(format!("arg::{}", arg.get_id())))
        .tag(Some(
            arg.get_help_heading()
                .unwrap_or("Options")
                .to_owned()
                .into(),
        ))
        .display_order(Some(arg.get_display_order()))
        .hide(arg.is_hide_set())
}

/// Get the possible values for completion
fn possible_values(
    a: &clap::Arg,
) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue> + '_>> {
    if !a.get_num_args().expect("built").takes_values() {
        None
    } else {
        a.get_value_parser().possible_values()
    }
}

/// Gets subcommands of [`clap::Command`] in the form of `("name", "bin_name")`.
///
/// Subcommand `rustup toolchain install` would be converted to
/// `("install", "rustup toolchain install")`.
fn subcommands(p: &clap::Command) -> Vec<CompletionCandidate> {
    debug!("subcommands: name={}", p.get_name());
    debug!("subcommands: Has subcommands...{:?}", p.has_subcommands());
    p.get_subcommands()
        .flat_map(|sc| {
            sc.get_name_and_visible_aliases()
                .into_iter()
                .map(|s| populate_command_candidate(CompletionCandidate::new(s.to_owned()), p, sc))
                .chain(sc.get_aliases().map(|s| {
                    populate_command_candidate(CompletionCandidate::new(s.to_owned()), p, sc)
                        .hide(true)
                }))
        })
        .collect()
}

fn populate_command_candidate(
    candidate: CompletionCandidate,
    cmd: &clap::Command,
    subcommand: &clap::Command,
) -> CompletionCandidate {
    candidate
        .help(subcommand.get_about().cloned())
        .id(Some(format!("command::{}", subcommand.get_name())))
        .tag(Some(
            cmd.get_subcommand_help_heading()
                .unwrap_or("Commands")
                .to_owned()
                .into(),
        ))
        .display_order(Some(subcommand.get_display_order()))
        .hide(subcommand.is_hide_set())
}

/// Parse the short flags and find the first `takes_values` option.
///
/// The returned bool is `true` only when every scanned cluster member was a
/// recognized option. Anything after the first value-taking option is the
/// option's value and therefore not scanned; invalid UTF-8 and unknown short
/// characters make the result `false`.
fn parse_shortflags<'c, 's>(
    cmd: &'c clap::Command,
    mut short: clap_lex::ShortFlags<'s>,
) -> (
    String,
    Option<&'c clap::Arg>,
    clap_lex::ShortFlags<'s>,
    bool,
) {
    let takes_value_opt;
    let mut leading_flags = String::new();
    let mut cluster_valid = true;
    // Find the first takes_values option.
    loop {
        match short.next_flag() {
            Some(Ok(opt)) => {
                leading_flags.push(opt);
                let found = cmd.get_arguments().find(|a| {
                    let shorts = a.get_short_and_visible_aliases();
                    let is_find = shorts.map(|v| {
                        let mut iter = v.into_iter();
                        let c = iter.find(|c| *c == opt);
                        c.is_some()
                    });
                    is_find.unwrap_or(false)
                });
                if let Some(opt) = found {
                    if opt.get_num_args().expect("built").takes_values() {
                        takes_value_opt = Some(opt);
                        break;
                    }
                } else {
                    cluster_valid = false;
                }
            }
            Some(Err(_)) => {
                cluster_valid = false;
                takes_value_opt = None;
                break;
            }
            None => {
                takes_value_opt = None;
                break;
            }
        }
    }

    (leading_flags, takes_value_opt, short, cluster_valid)
}

/// Parse the positional arguments. Return the new state and the new positional index.
fn parse_positional<'a>(
    cmd: &clap::Command,
    pos_index: usize,
    is_escaped: bool,
    state: ParseState<'a>,
) -> (ParseState<'a>, usize) {
    let pos_arg = cmd
        .get_positionals()
        .find(|p| p.get_index() == Some(pos_index));
    let num_args = pos_arg
        .and_then(|a| a.get_num_args().map(|r| r.max_values()))
        .unwrap_or(1);

    let update_state_with_new_positional = |pos_index| -> (ParseState<'a>, usize) {
        if num_args > 1 {
            (ParseState::Pos((pos_index, 1)), pos_index)
        } else {
            if is_escaped {
                (ParseState::Pos((pos_index, 1)), pos_index + 1)
            } else {
                (ParseState::ValueDone, pos_index + 1)
            }
        }
    };
    match state {
        ParseState::ValueDone => {
            update_state_with_new_positional(pos_index)
        },
        ParseState::Pos((prev_pos_index, num_arg)) => {
            if prev_pos_index == pos_index {
                if num_arg + 1 < num_args {
                    (ParseState::Pos((pos_index, num_arg + 1)), pos_index)
                } else {
                    if is_escaped {
                        (ParseState::Pos((pos_index, 1)), pos_index + 1)
                    } else {
                        (ParseState::ValueDone, pos_index + 1)
                    }
                }
            } else {
                update_state_with_new_positional(pos_index)
            }
        }
        ParseState::Opt(..) => unreachable!(
            "This branch won't be hit,
            because ParseState::Opt should not be seen as a positional argument and passed to this function."
        ),
    }
}

/// Parse optional flag argument. Return new state
fn parse_opt_value(opt: &clap::Arg, count: usize) -> ParseState<'_> {
    let range = opt.get_num_args().expect("built");
    let max = range.max_values();
    if count < max {
        ParseState::Opt((opt, count + 1))
    } else {
        ParseState::ValueDone
    }
}

fn pos_allows_hyphen(cmd: &clap::Command, pos_index: usize) -> bool {
    cmd.get_positionals()
        .find(|a| a.get_index() == Some(pos_index))
        .map(|p| p.is_allow_hyphen_values_set())
        .unwrap_or(false)
}

fn opt_allows_hyphen(state: &ParseState<'_>, arg: &clap_lex::ParsedArg<'_>) -> bool {
    let val = arg.to_value_os();
    if val.starts_with("-") {
        if let ParseState::Opt((opt, _)) = state {
            return opt.is_allow_hyphen_values_set();
        }
    }

    false
}

/// Arguments that must not be offered because they conflict with an explicitly
/// present option. The present options themselves are never disabled.
fn gather_disabled_args(cmd: &clap::Command, explicit: &HashSet<Id>) -> HashSet<Id> {
    let mut disabled = HashSet::new();
    if explicit.is_empty() {
        return disabled;
    }
    for arg in cmd.get_arguments() {
        if explicit.contains(arg.get_id()) {
            // A present option hides everything it conflicts with ...
            for id in arg_direct_conflicts(cmd, arg) {
                if !explicit.contains(&id) {
                    disabled.insert(id);
                }
            }
        } else {
            // ... and is hidden when either side declares the conflict.
            if arg_direct_conflicts(cmd, arg)
                .iter()
                .any(|id| explicit.contains(id))
            {
                disabled.insert(arg.get_id().clone());
            }
        }
    }
    disabled
}

/// Conflict partners of `arg`, following the same rules as clap's parser:
/// - the argument's own `conflicts_with` / `conflicts_with_all`
/// - `conflicts_with` / `conflicts_with_all` of every group the argument belongs to
/// - the other members of non-`multiple` groups
///
/// Group ids are expanded to their argument members.
/// `overrides_with` relationships are intentionally not considered.
fn arg_direct_conflicts(cmd: &clap::Command, arg: &clap::Arg) -> HashSet<Id> {
    let mut conflicts = arg
        .get_conflicts_with()
        .map(ToOwned::to_owned)
        .collect::<HashSet<_>>();
    for group in cmd.get_groups() {
        let members = unroll_group(cmd, group.get_id());
        if !members.iter().any(|m| m == arg.get_id()) {
            continue;
        }
        conflicts.extend(group.get_conflicts_with().map(ToOwned::to_owned));
        if !group.is_multiple_set() {
            conflicts.extend(members.into_iter().filter(|m| m != arg.get_id()));
        }
    }
    conflicts
        .iter()
        .flat_map(|id| unroll_group(cmd, id))
        .collect()
}

/// Resolve an argument or group id to its argument members.
/// Unknown ids resolve to themselves.
fn unroll_group(cmd: &clap::Command, id: &Id) -> Vec<Id> {
    let mut pending = vec![id.clone()];
    let mut args = Vec::new();
    while let Some(id) = pending.pop() {
        if cmd.get_arguments().any(|a| a.get_id() == &id) {
            if !args.contains(&id) {
                args.push(id);
            }
        } else if let Some(group) = cmd.get_groups().find(|g| g.get_id() == &id) {
            pending.extend(group.get_args().map(ToOwned::to_owned));
        } else if !args.contains(&id) {
            args.push(id);
        }
    }
    args
}
