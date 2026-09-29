use std::ffi::OsStr;
use std::ffi::OsString;

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

    let mut current_cmd = &*cmd;
    if cmd.is_multicall_set() {
        // A multicall executable dispatches on the file name of argv0 (the
        // "applet"), matching clap's own parser behavior. When argv0 is the
        // multicall launcher itself (e.g. `busybox`, itself a registered
        // subcommand) the following argument selects the applet; an
        // unrecognized invocation name yields no candidates rather than
        // falling back to the root command.
        let applet = raw_args
            .next_os(&mut cursor)
            .and_then(|argv0| std::path::Path::new(argv0).file_stem().map(OsStr::to_owned));
        if let Some(applet) = applet.as_deref().and_then(OsStr::to_str) {
            if let Some(subcmd) = cmd.find_subcommand(applet) {
                debug!("complete: multicall applet={applet:?}");
                current_cmd = subcmd;
            } else {
                debug!("complete: unrecognized multicall applet={applet:?}");
                return Ok(Vec::new());
            }
        } else {
            debug!("complete: multicall without an applet name");
            return Ok(Vec::new());
        }
    } else if !cmd.is_no_binary_name_set() {
        raw_args.next_os(&mut cursor);
    }
    let mut pos_index = 1;
    let mut is_escaped = false;
    let mut next_state = ParseState::ValueDone;
    // Options that already took their values on this command line, used to
    // reject a repeated occurrence of a non-repeatable option.
    let mut seen_options = std::collections::HashSet::<String>::new();
    while let Some(arg) = raw_args.next(&mut cursor) {
        let current_state = next_state;
        next_state = ParseState::ValueDone;
        debug!(
            "complete::next: arg={:?}, current_state={current_state:?}, cursor={cursor:?}",
            arg.to_value_os(),
        );
        if cursor == target_cursor {
            return complete_arg(
                &arg,
                current_cmd,
                current_dir,
                pos_index,
                is_escaped,
                current_state,
                &seen_options,
            );
        }

        if let Ok(value) = arg.to_value() {
            if let Some(next_cmd) = current_cmd.find_subcommand(value) {
                current_cmd = next_cmd;
                pos_index = 1;
                seen_options.clear();
                continue;
            }
        }

        // An option whose value is required cannot be followed by another
        // option or the escape token; the missing value makes the whole
        // command line invalid, so nothing can be completed. An option whose
        // value is optional instead drops its pending state and the token is
        // processed on its own.
        if !is_escaped && looks_like_option(&arg) {
            if let ParseState::Opt((opt, count)) = &current_state {
                let min = opt.get_num_args().map(|r| r.min_values()).unwrap_or(0);
                if *count <= min && min > 0 && !opt.is_allow_hyphen_values_set() {
                    debug!(
                        "complete: missing required value for opt={:?}",
                        opt.get_id()
                    );
                    return Ok(Vec::new());
                }
            }
        }

        if is_escaped {
            let positional = positional_at(current_cmd, pos_index);
            if let Some(positional) = positional {
                if !is_valid_value(positional, arg.to_value_os()) {
                    debug!(
                        "complete: invalid value={:?} for positional={:?}",
                        arg.to_value_os(),
                        positional.get_id()
                    );
                    return Ok(Vec::new());
                }
            }
            (next_state, pos_index) =
                parse_positional(current_cmd, pos_index, is_escaped, current_state);
        } else if arg.is_escape() {
            is_escaped = true;
        } else if opt_allows_hyphen(&current_state, &arg) {
            match current_state {
                ParseState::Opt((opt, count)) => {
                    if !is_valid_value(opt, arg.to_value_os()) {
                        debug!(
                            "complete: invalid value={:?} for opt={:?}",
                            arg.to_value_os(),
                            opt.get_id()
                        );
                        return Ok(Vec::new());
                    }
                    next_state = parse_opt_value(opt, count);
                }
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
                    let takes_values = opt.get_num_args().expect("built").takes_values();
                    if takes_values {
                        if !can_repeat(opt) && !seen_options.insert(opt.get_id().to_string()) {
                            debug!("complete: repeated non-repeatable opt={flag:?}");
                            return Ok(Vec::new());
                        }
                        if let Some(value) = value {
                            if !is_valid_value(opt, value) {
                                debug!("complete: invalid value={value:?} for opt={flag:?}");
                                return Ok(Vec::new());
                            }
                        }
                    }
                    if takes_values && value.is_none() {
                        next_state = ParseState::Opt((opt, 1));
                    };
                } else if pos_allows_hyphen(current_cmd, pos_index) {
                    let positional = positional_at(current_cmd, pos_index);
                    if let Some(positional) = positional {
                        if !is_valid_value(positional, arg.to_value_os()) {
                            debug!(
                                "complete: invalid value={:?} for positional={:?}",
                                arg.to_value_os(),
                                positional.get_id()
                            );
                            return Ok(Vec::new());
                        }
                    }
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, current_state);
                } else {
                    debug!("complete: unknown flag={flag:?}");
                    return Ok(Vec::new());
                }
            }
        } else if let Some(short) = arg.to_short() {
            let (_, takes_value_opt, mut short) = parse_shortflags(current_cmd, short);
            if let Some(opt) = takes_value_opt {
                if !can_repeat(opt) && !seen_options.insert(opt.get_id().to_string()) {
                    debug!("complete: repeated non-repeatable opt={:?}", opt.get_id());
                    return Ok(Vec::new());
                }
                // Consume an optional `=`, matching how inline short values are
                // split elsewhere, so `-F=json` validates `json` rather than
                // `=json`.
                let mut peek_short = short.clone();
                if let Some(Ok('=')) = peek_short.next_flag() {
                    short.next_flag();
                }
                if let Some(value) = short.next_value_os() {
                    if !is_valid_value(opt, value) {
                        debug!(
                            "complete: invalid value={value:?} for opt={:?}",
                            opt.get_id()
                        );
                        return Ok(Vec::new());
                    }
                } else {
                    next_state = ParseState::Opt((opt, 1));
                }
            } else if pos_allows_hyphen(current_cmd, pos_index) {
                let positional = positional_at(current_cmd, pos_index);
                if let Some(positional) = positional {
                    if !is_valid_value(positional, arg.to_value_os()) {
                        debug!(
                            "complete: invalid value={:?} for positional={:?}",
                            arg.to_value_os(),
                            positional.get_id()
                        );
                        return Ok(Vec::new());
                    }
                }
                (next_state, pos_index) =
                    parse_positional(current_cmd, pos_index, is_escaped, current_state);
            }
        } else {
            match current_state {
                ParseState::ValueDone | ParseState::Pos(..) => {
                    let positional = positional_at(current_cmd, pos_index);
                    let Some(positional) = positional else {
                        debug!("complete: unrecognized argument={:?}", arg.to_value_os());
                        return Ok(Vec::new());
                    };
                    if !is_valid_value(positional, arg.to_value_os()) {
                        debug!(
                            "complete: invalid value={:?} for positional={:?}",
                            arg.to_value_os(),
                            positional.get_id()
                        );
                        return Ok(Vec::new());
                    }
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, current_state);
                }
                ParseState::Opt((opt, count)) => {
                    if !is_valid_value(opt, arg.to_value_os()) {
                        debug!(
                            "complete: invalid value={:?} for opt={:?}",
                            arg.to_value_os(),
                            opt.get_id()
                        );
                        return Ok(Vec::new());
                    }
                    next_state = parse_opt_value(opt, count);
                }
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
    seen_options: &std::collections::HashSet<String>,
) -> Result<Vec<CompletionCandidate>, std::io::Error> {
    debug!(
        "complete_arg: arg={:?}, cmd={:?}, current_dir={:?}, pos_index={:?}, state={:?}",
        arg,
        cmd.get_name(),
        current_dir,
        pos_index,
        state
    );

    // A token under the cursor may itself repeat an option that already took a
    // value; for a non-repeatable option this is invalid and yields nothing.
    if !is_escaped && matches!(state, ParseState::ValueDone | ParseState::Pos(..)) {
        if let Some(opt) = cursor_option(cmd, arg) {
            if opt.get_num_args().expect("built").takes_values()
                && !can_repeat(opt)
                && seen_options.contains(&opt.get_id().to_string())
            {
                debug!(
                    "complete: repeated non-repeatable opt={:?} at cursor",
                    opt.get_id()
                );
                return Ok(Vec::new());
            }
        }
    }

    // A cursor token carrying an attached long value (`--flag=value`) is
    // completed by that option alone, exactly like the independent
    // `--flag value` spelling. Handling it before the regular state match
    // keeps positional, subcommand and option candidates from leaking in
    // based on the whole token.
    let mut state = state;

    // Mirror the committed-token rule for the token under the cursor: when it
    // itself looks like an option, a pending optional value is omitted (the
    // token is processed on its own), while a missing required value
    // invalidates the command line.
    if let ParseState::Opt((opt, count)) = &state {
        if !is_escaped && !opt.is_allow_hyphen_values_set() && looks_like_option(arg) {
            let min = opt.get_num_args().expect("built").min_values();
            if min > 0 && *count <= min {
                debug!(
                    "complete: missing required value for opt={:?} at cursor",
                    opt.get_id()
                );
                return Ok(Vec::new());
            }
            if min == 0 {
                debug!(
                    "complete: omitting optional value of opt={:?} at cursor",
                    opt.get_id()
                );
                state = ParseState::ValueDone;
            }
        }
    }

    let attached_long_value = (!is_escaped
        && matches!(state, ParseState::ValueDone | ParseState::Pos(..)))
    .then(|| cursor_attached_long_value(cmd, arg))
    .flatten();

    let mut completions = Vec::<CompletionCandidate>::new();

    if let Some((opt, value)) = attached_long_value {
        completions.extend(complete_arg_value(
            value.to_str().ok_or(value),
            opt,
            current_dir,
            0,
        ));
    } else {
        match state {
            ParseState::ValueDone => {
                // After `--`, subcommands are no longer recognized; every token is
                // parsed as a positional value.
                if !is_escaped {
                    if let Ok(value) = arg.to_value() {
                        completions.extend(complete_subcommand(value, cmd));
                    }
                }

                if let Some(positional) = positional_at(cmd, pos_index) {
                    completions.extend(complete_arg_value(
                        arg.to_value(),
                        positional,
                        current_dir,
                        0,
                    ));
                }
                if !is_escaped {
                    completions.extend(complete_option(arg, cmd, current_dir));
                }
            }
            ParseState::Pos((_, num_arg)) => {
                if let Some(positional) = positional_at(cmd, pos_index) {
                    completions.extend(complete_arg_value(
                        arg.to_value(),
                        positional,
                        current_dir,
                        num_arg.saturating_sub(1),
                    ));
                    if !is_escaped
                        && positional
                            .get_num_args()
                            .is_some_and(|num_args| num_arg >= num_args.min_values())
                    {
                        completions.extend(complete_option(arg, cmd, current_dir));
                    }
                }
            }
            ParseState::Opt((opt, count)) => {
                completions.extend(complete_arg_value(
                    arg.to_value(),
                    opt,
                    current_dir,
                    count.saturating_sub(1),
                ));
                let min = opt.get_num_args().map(|r| r.min_values()).unwrap_or(0);
                // An option whose value is optional (a minimum of zero values)
                // always serves its own value at the first value slot; falling back
                // to flags and positionals here would let `--color <TAB>` mix
                // color values with unrelated candidates. Options that require at
                // least one value keep offering the alternatives once their
                // required values have been supplied.
                if count > min && min > 0 {
                    // Also complete this raw_arg as a positional argument, flags, options and subcommand.
                    completions.extend(complete_arg(
                        arg,
                        cmd,
                        current_dir,
                        pos_index,
                        is_escaped,
                        ParseState::ValueDone,
                        seen_options,
                    )?);
                }
            }
        }
    }
    if completions.iter().any(|a| !a.is_hide_set()) {
        completions.retain(|a| !a.is_hide_set());
    }
    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_values = std::collections::HashSet::new();
    completions.retain(move |a| {
        if let Some(id) = a.get_id().cloned() {
            seen_ids.insert(id)
        } else {
            // Candidates without an id are deduplicated by value, keeping the
            // first occurrence
            seen_values.insert(a.get_value().to_owned())
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
) -> Vec<CompletionCandidate> {
    debug!("complete_option: arg={arg:?}, current_dir={current_dir:?}");
    let mut completions = Vec::<CompletionCandidate>::new();
    if arg.is_empty() {
        completions.extend(longs_and_visible_aliases(cmd));
        completions.extend(hidden_longs_aliases(cmd));

        let dash_or_arg = if arg.is_empty() {
            "-".into()
        } else {
            arg.to_value_os().to_string_lossy()
        };
        completions.extend(
            shorts_and_visible_aliases(cmd)
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
            shorts_and_visible_aliases(cmd)
                .into_iter()
                .map(|comp| comp.add_prefix(dash_or_arg.to_string())),
        );

        completions.extend(longs_and_visible_aliases(cmd));
        completions.extend(hidden_longs_aliases(cmd));
    } else if arg.is_escape() {
        // HACK: Assuming knowledge of is_escape
        completions.extend(longs_and_visible_aliases(cmd));
        completions.extend(hidden_longs_aliases(cmd));
    } else if let Some((flag, value)) = arg.to_long() {
        if let Ok(flag) = flag {
            if let Some(value) = value {
                if let Some(arg) = cmd.get_arguments().find(|a| a.get_long() == Some(flag)) {
                    // The attached (`--flag=value`) and independent
                    // (`--flag value`) spellings share one candidate
                    // semantics: the candidates name the value alone, never
                    // the `--flag=` prefix. The token under the cursor is left
                    // untouched, so callers that want the prefix reattached
                    // (e.g. shell adapters replacing the whole word) can do so
                    // themselves from that token.
                    completions.extend(complete_arg_value(
                        value.to_str().ok_or(value),
                        arg,
                        current_dir,
                        0,
                    ));
                }
            } else {
                completions.extend(
                    longs_and_visible_aliases(cmd)
                        .into_iter()
                        .filter(|comp| comp.get_value().starts_with(format!("--{flag}").as_str())),
                );
                completions.extend(
                    hidden_longs_aliases(cmd)
                        .into_iter()
                        .filter(|comp| comp.get_value().starts_with(format!("--{flag}").as_str())),
                );
            }
        }
    } else if let Some(short) = arg.to_short() {
        if !short.is_negative_number() {
            // Find the first takes_values option.
            let (leading_flags, takes_value_opt, mut short) = parse_shortflags(cmd, short);

            // Clone `short` to `peek_short` to peek whether the next flag is a `=`.
            if let Some(opt) = takes_value_opt {
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
            } else {
                completions.extend(
                    shorts_and_visible_aliases(cmd)
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
fn longs_and_visible_aliases(p: &clap::Command) -> Vec<CompletionCandidate> {
    debug!("longs: name={}", p.get_name());

    p.get_arguments()
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
fn hidden_longs_aliases(p: &clap::Command) -> Vec<CompletionCandidate> {
    debug!("longs: name={}", p.get_name());

    p.get_arguments()
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
fn shorts_and_visible_aliases(p: &clap::Command) -> Vec<CompletionCandidate> {
    debug!("shorts: name={}", p.get_name());

    p.get_arguments()
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
fn parse_shortflags<'c, 's>(
    cmd: &'c clap::Command,
    mut short: clap_lex::ShortFlags<'s>,
) -> (String, Option<&'c clap::Arg>, clap_lex::ShortFlags<'s>) {
    let takes_value_opt;
    let mut leading_flags = String::new();
    // Find the first takes_values option.
    loop {
        match short.next_flag() {
            Some(Ok(opt)) => {
                leading_flags.push(opt);
                let opt = cmd.get_arguments().find(|a| {
                    let shorts = a.get_short_and_visible_aliases();
                    let is_find = shorts.map(|v| {
                        let mut iter = v.into_iter();
                        let c = iter.find(|c| *c == opt);
                        c.is_some()
                    });
                    is_find.unwrap_or(false)
                });
                if opt
                    .map(|o| o.get_num_args().expect("built").takes_values())
                    .unwrap_or(false)
                {
                    takes_value_opt = opt;
                    break;
                }
            }
            Some(Err(_)) | None => {
                takes_value_opt = None;
                break;
            }
        }
    }

    (leading_flags, takes_value_opt, short)
}

/// Parse the positional arguments. Return the new state and the new positional index.
fn parse_positional<'a>(
    cmd: &clap::Command,
    pos_index: usize,
    is_escaped: bool,
    state: ParseState<'a>,
) -> (ParseState<'a>, usize) {
    let pos_arg = positional_at(cmd, pos_index);
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
    positional_at(cmd, pos_index)
        .map(|p| p.is_allow_hyphen_values_set())
        .unwrap_or(false)
}

/// Resolve the positional argument that owns `pos_index`.
///
/// Positional indices mark the first token an argument claims; a repeatable
/// trailing positional ([`clap::ArgAction::Append`]) keeps claiming every
/// following position on each new occurrence, so an exact lookup misses its
/// later values.
fn positional_at(cmd: &clap::Command, pos_index: usize) -> Option<&clap::Arg> {
    let positionals: Vec<_> = cmd.get_positionals().collect();
    if let Some(found) = positionals
        .iter()
        .copied()
        .find(|p| p.get_index() == Some(pos_index))
    {
        return Some(found);
    }
    positionals
        .iter()
        .copied()
        .filter(|p| p.get_index().is_some_and(|i| i <= pos_index))
        .max_by_key(|p| p.get_index())
        .filter(|p| can_repeat(p) && p.get_num_args().expect("built").takes_values())
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

/// Whether a committed token looks like an option or the escape token rather
/// than a plain value.
fn looks_like_option(arg: &clap_lex::ParsedArg<'_>) -> bool {
    arg.is_escape()
        || arg.to_long().is_some_and(|(flag, _)| flag.is_ok())
        || arg
            .to_short()
            .is_some_and(|short| !short.is_negative_number())
}

/// Whether the argument may occur more than once on a command line, taking an
/// additional value each time.
fn can_repeat(arg: &clap::Arg) -> bool {
    matches!(
        arg.get_action(),
        clap::ArgAction::Append | clap::ArgAction::Count
    )
}

/// Resolve the option named by a cursor token, if it is a recognizable long or
/// short option.
fn cursor_option<'c>(
    cmd: &'c clap::Command,
    arg: &clap_lex::ParsedArg<'_>,
) -> Option<&'c clap::Arg> {
    if let Some((flag, _)) = arg.to_long() {
        let flag = flag.ok()?;
        cmd.get_arguments().find(|a| {
            a.get_long_and_visible_aliases()
                .map(|longs| longs.into_iter().any(|long| long == flag))
                .unwrap_or(false)
        })
    } else if let Some(short) = arg.to_short() {
        let (_, opt, _) = parse_shortflags(cmd, short);
        opt
    } else {
        None
    }
}

/// Resolve the option and attached value of a cursor token shaped
/// `--flag=value` (the value may be empty).
fn cursor_attached_long_value<'c, 's>(
    cmd: &'c clap::Command,
    arg: &clap_lex::ParsedArg<'s>,
) -> Option<(&'c clap::Arg, &'s OsStr)> {
    let (flag, value) = arg.to_long()?;
    let flag = flag.ok()?;
    let value = value?;
    let opt = cmd.get_arguments().find(|a| {
        a.get_long_and_visible_aliases()
            .map(|longs| longs.into_iter().any(|long| long == flag))
            .unwrap_or(false)
    })?;
    if !opt.get_num_args().expect("built").takes_values() {
        return None;
    }
    Some((opt, value))
}

/// Validate a value already committed on the command line (not the token under
/// the cursor). Unlike the token being completed, a finished value must match
/// one of the argument's possible values exactly; an unfinished or illegal
/// value invalidates the whole command line and yields no candidates.
fn is_valid_value(arg: &clap::Arg, value: &OsStr) -> bool {
    let Some(possible) = possible_values(arg) else {
        return true;
    };
    let Some(value) = value.to_str() else {
        return false;
    };
    let possible: Vec<clap::builder::PossibleValue> = possible.collect();
    let matches = |part: &str| {
        possible
            .iter()
            .flat_map(|p| p.get_name_and_aliases())
            .any(|name| name == part)
    };
    match arg.get_value_delimiter() {
        Some(delimiter) => value.split(delimiter).all(matches),
        None => matches(value),
    }
}
