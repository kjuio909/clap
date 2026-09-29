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
    // Text of the value terminator carried by the option whose terminator was
    // consumed as the previous closed word. It only lives for one word: a second
    // terminator is a duplicate (and therefore illegal) while any other word
    // resumes normal parsing.
    let mut last_terminator: Option<OsString> = None;
    // Set when an already closed word carried a value that cannot be part of
    // the command line (a delimiter-separated value set that is overfull, has
    // an empty segment, ...). Completion then reports "no completion generated"
    // instead of guessing at unrelated candidates.
    let mut is_illegal = false;
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
            if is_illegal {
                return Err(std::io::Error::other("no completion generated"));
            }
            // A terminator typed immediately after the standalone terminator
            // would be a second one; it closes no value and cannot complete.
            if last_terminator
                .as_deref()
                .is_some_and(|term| word_is(term, &arg))
            {
                return Err(std::io::Error::other("no completion generated"));
            }
            let disabled = DisabledArgs {
                hidden: gather_disabled_args(current_cmd, &explicit_opts),
                present: explicit_opts.clone(),
            };
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

        // The terminator that ended the previous closed word, repeated here,
        // cannot close the occurrence a second time.
        let duplicate_terminator = last_terminator
            .as_deref()
            .is_some_and(|term| word_is(term, &arg));
        if duplicate_terminator {
            is_illegal = true;
        }

        if let Ok(value) = arg.to_value() {
            // clap's parser probes for a subcommand only outside an ongoing
            // option value or multi-value positional, and never after `--`.
            // A word is therefore left to its level when a multi-value
            // positional is being filled or an option is still taking values
            // (including a dangling delimiter segment). The one-word `closed`
            // window of an option with a value terminator already finished the
            // occurrence, so it probes like an ordinary level.
            let filling_positional = matches!(current_state, ParseState::Pos(..));
            let taking_option_value =
                matches!(&current_state, ParseState::Opt(state) if !state.closed);
            if !is_escaped && !filling_positional && !taking_option_value {
                if let Some(next_cmd) = select_subcommand(current_cmd, value) {
                    current_cmd = next_cmd;
                    pos_index = 1;
                    last_terminator = None;
                    continue;
                }
            }
        }

        // A standalone terminator word ends the option's value taking; it is
        // neither a value nor a candidate. It cannot arrive while a dangling
        // delimiter still expects a segment, before the option's minimum number
        // of value words is present, or after an earlier illegal word.
        let mut consumed_terminator: Option<OsString> = None;
        if let ParseState::Opt(state) = &current_state {
            if let Some(terminator) = state.opt.get_value_terminator() {
                if word_is(OsStr::new(terminator.as_str()), &arg) {
                    let min = state.opt.get_num_args().expect("built").min_values();
                    let consumed_words = state.word.saturating_sub(1);
                    if state.open || consumed_words < min || is_illegal {
                        is_illegal = true;
                    } else {
                        consumed_terminator = Some(OsString::from(terminator.as_str()));
                    }
                }
            }
        }

        // Once the value occurrence is complete, only the standalone terminator
        // still belongs to it; any other word resumes ordinary option/positional
        // parsing. This one-word window makes the terminator work identically
        // across `=`, separate words, shorts and aliases.
        let mut dispatch_state = if matches!(&current_state, ParseState::Opt(state) if state.closed)
            && consumed_terminator.is_none()
        {
            ParseState::ValueDone
        } else {
            current_state
        };

        // A closed word arriving while an option still takes value words:
        // clap consumes every word as the value while the value is mandatory
        // and rejects option-shaped words (a missing value) as well as
        // unknown values; once the minimum number is met, an option word or
        // `--` may stop the occurrence and resumes ordinary parsing instead.
        if let ParseState::Opt(state) = &dispatch_state {
            if consumed_terminator.is_none() && !state.closed {
                match classify_closed_value_word(state, &arg) {
                    ClosedValueWord::Value => {}
                    ClosedValueWord::Illegal => is_illegal = true,
                    ClosedValueWord::ResumeParsing => {
                        dispatch_state = ParseState::ValueDone;
                    }
                }
            }
        }

        if is_escaped {
            let accepted = closed_positional_is_accepted(current_cmd, pos_index, arg.to_value_os());
            (next_state, pos_index) =
                parse_positional(current_cmd, pos_index, is_escaped, dispatch_state);
            if !accepted {
                // After `--`, clap accepts only positional values (and
                // directory entries for path arguments); a word the slot
                // rejects makes the whole line uncompletable.
                is_illegal = true;
            }
        } else if arg.is_escape() {
            // `classify_closed_value_word` already rejected a `--` that arrives
            // while a value is mandatory (and rewrote the state otherwise).
            is_escaped = true;
        } else if consumed_terminator.is_some() {
            next_state = ParseState::ValueDone;
        } else if opt_allows_hyphen(&dispatch_state, &arg) {
            match dispatch_state {
                ParseState::Opt(state) => {
                    next_state = advance_opt_value(state, arg.to_value_os(), &mut is_illegal);
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
                    // A non-repeatable option used twice (across words) cannot
                    // be part of a valid command line.
                    if !arg_is_repeatable(opt) && explicit_opts.contains(opt.get_id()) {
                        is_illegal = true;
                    }
                    explicit_opts.insert(opt.get_id().clone());
                    if opt.get_num_args().expect("built").takes_values() {
                        match value {
                            // A bare `--name` still expects its value as the next word.
                            None => next_state = ParseState::Opt(OptState::new(opt, 1)),
                            // A closed `--name=value` word normally ends the
                            // occurrence, but a dangling value delimiter with
                            // room for another segment keeps accepting values.
                            Some(value) => {
                                next_state = attached_opt_state(opt, value, &mut is_illegal);
                            }
                        }
                    }
                } else if pos_allows_hyphen(current_cmd, pos_index) {
                    let accepted =
                        closed_positional_is_accepted(current_cmd, pos_index, arg.to_value_os());
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, dispatch_state);
                    if !accepted {
                        is_illegal = true;
                    }
                } else {
                    // A closed word with a `--name`/`--name=value` shape that
                    // names no option cannot be part of a valid command line;
                    // clap reports an unknown argument.
                    is_illegal = true;
                }
            } else if !pos_allows_hyphen(current_cmd, pos_index) {
                is_illegal = true;
            }
        } else if let Some(short) = arg.to_short() {
            if short.is_negative_number() {
                // A number-shaped word only fills the waiting option when it
                // accepts it; otherwise it is either a negative positional
                // value or, like clap, an unknown argument, never a cluster.
                match dispatch_state {
                    ParseState::Opt(state) => {
                        next_state = advance_opt_value(state, arg.to_value_os(), &mut is_illegal);
                    }
                    _ => {
                        if pos_allows_negative(current_cmd, pos_index) {
                            let accepted = closed_positional_is_accepted(
                                current_cmd,
                                pos_index,
                                arg.to_value_os(),
                            );
                            (next_state, pos_index) = parse_positional(
                                current_cmd,
                                pos_index,
                                is_escaped,
                                dispatch_state,
                            );
                            if !accepted {
                                is_illegal = true;
                            }
                        } else {
                            is_illegal = true;
                        }
                    }
                }
            } else {
                let cluster = scan_short_cluster(current_cmd, short);
                let structurally_valid = cluster.flags_valid && !cluster.duplicate;

                // A positional that accepts hyphenated values swallows a word
                // that is not entirely made of recognized flags; clap's parser
                // treats it as the positional value.
                if !structurally_valid && pos_allows_hyphen(current_cmd, pos_index) {
                    let accepted =
                        closed_positional_is_accepted(current_cmd, pos_index, arg.to_value_os());
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, dispatch_state);
                    if !accepted {
                        is_illegal = true;
                    }
                } else if !structurally_valid {
                    // An unknown member, an empty cluster or a repeated
                    // non-repeatable flag makes the cluster illegal as a whole:
                    // never split it or accept part of it.
                    is_illegal = true;
                } else {
                    // Every flag of the recognized cluster was explicitly
                    // supplied; a repeated non-repeatable option is rejected.
                    handle_closed_cluster(&cluster, &mut explicit_opts, &mut is_illegal);
                    if let Some(opt) = cluster.takes_value_opt {
                        let mut value_flags = cluster.value_flags.clone();
                        match value_flags.next_value_os() {
                            // A closed `-ovalue` / `-o=value` word; advance like
                            // the long `--opt=value` form.
                            Some(remainder) => {
                                let value = remainder.strip_prefix("=").unwrap_or(remainder);
                                next_state = attached_opt_state(opt, value, &mut is_illegal);
                            }
                            // A bare `-o` still expects its value next word.
                            None => next_state = ParseState::Opt(OptState::new(opt, 1)),
                        }
                    }
                }
            }
        } else {
            match dispatch_state {
                ParseState::ValueDone | ParseState::Pos(..) => {
                    let accepted =
                        closed_positional_is_accepted(current_cmd, pos_index, arg.to_value_os());
                    (next_state, pos_index) =
                        parse_positional(current_cmd, pos_index, is_escaped, dispatch_state);
                    if !accepted {
                        // The word names no subcommand and the current
                        // positional slot does not accept it, so clap would
                        // reject the line. Completion must not guess a split or
                        // keep offering parent-level candidates.
                        is_illegal = true;
                    }
                }
                ParseState::Opt(state) => {
                    next_state = advance_opt_value(state, arg.to_value_os(), &mut is_illegal);
                }
            }
        }

        // Remember a just-consumed terminator for exactly one word so a
        // repeated terminator is flagged as a duplicate; every other word
        // clears it and resumes ordinary parsing.
        last_terminator = consumed_terminator;
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
    Opt(OptState<'a>),
}

/// Ongoing state while an option still accepts value words.
#[derive(Debug, PartialEq, Eq, Clone)]
struct OptState<'a> {
    opt: &'a clap::Arg,
    /// Ordinal of the value word about to be consumed within this occurrence.
    /// A bare `--name` waiting for its first value starts at `1`; this is the
    /// value handed to [`ArgValueCompleter::complete_at`] as `arg_index + 1`,
    /// so it counts shell words and is unaffected by the value delimiter.
    word: usize,
    /// Delimiter segments already closed by earlier value words, in order.
    used: Vec<OsString>,
    /// The state was reached through a dangling value delimiter, so the next
    /// word is another segment of this option rather than an optional stop.
    open: bool,
    /// The value occurrence itself is complete (an attached word or a full
    /// delimiter set), so ordinary parsing resumes, but the option's standalone
    /// terminator may still arrive as the very next word. This makes the
    /// terminator work identically whether values arrived through `=`, a
    /// separate word, a short flag or an alias.
    closed: bool,
}

impl<'a> OptState<'a> {
    fn new(opt: &'a clap::Arg, word: usize) -> Self {
        Self {
            opt,
            word,
            used: Vec::new(),
            open: false,
            closed: false,
        }
    }
}

/// Whether an option carries several values split by a delimiter inside one
/// shell word *and* bounds how many such values it accepts.
///
/// An argument with the default range of one value may still pack an arbitrary
/// number of delimiter segments into its single word (clap's parser allows it),
/// and completion preserves that legacy behavior; only multi-value arguments
/// count segments against their range.
fn is_bounded_delimited(opt: &clap::Arg) -> Option<char> {
    let delim = opt.get_value_delimiter()?;
    if opt.get_num_args().expect("built").max_values() > 1 {
        Some(delim)
    } else {
        None
    }
}

/// Whether a terminator glued into a value word is structurally impossible for
/// this option. clap rejects such a word when the option packs a bounded,
/// delimiter-separated set from fixed possible values (a segment like `;x` can
/// never name a value), while a free-form option simply takes the whole word.
fn glued_terminator_is_illegal(opt: &clap::Arg) -> bool {
    is_bounded_delimited(opt).is_some() && possible_values(opt).is_some()
}

/// Split a shell value on its delimiter into its closed segments.
///
/// Returns `(segments, dangling)` where `dangling` is set when the value ends
/// with the delimiter: the final empty piece is the segment still being typed
/// rather than a closed (empty and therefore invalid) segment. Splitting works
/// on raw [`OsStr`] bytes, so invalid UTF-8 values are handled too.
fn split_delimited(value: &OsStr, delim: char) -> (Vec<&OsStr>, bool) {
    let mut buf = [0_u8; 4];
    let delim_str = delim.encode_utf8(&mut buf);
    let mut segments: Vec<&OsStr> = value.split(delim_str).collect();
    let dangling = value.contains(delim_str) && segments.last().is_some_and(|last| last.is_empty());
    if dangling {
        segments.pop();
    }
    (segments, dangling)
}

/// Fold one closed value word into an option's ongoing value state.
///
/// `Ok(None)` means the occurrence is complete, `Ok(Some(state))` that another
/// value word is accepted and `Err(())` that the word cannot be part of a valid
/// command line (an empty delimiter segment or more segments than the option
/// accepts); the latter produces "no completion generated" rather than guesses.
fn consume_opt_word<'a>(
    prior: OptState<'a>,
    value: &OsStr,
    attached: bool,
) -> Result<Option<OptState<'a>>, ()> {
    let opt = prior.opt;
    let range = opt.get_num_args().expect("built");
    let max = range.max_values();
    let ordinal = prior.word;
    let mut used = prior.used;

    let Some(delim) = is_bounded_delimited(opt) else {
        // Legacy behavior: count whole shell words against the range; whatever
        // is packed behind a delimiter does not extend or shorten the count.
        // A closed word that names none of the option's fixed values (an
        // unknown value, an empty value, or text glued onto a complete value
        // such as `3v`) cannot be part of a valid command line. Free-form
        // options keep accepting every word.
        if !closed_word_value_is_known(opt, value) {
            return Err(());
        }
        // An attached value always closes the occurrence.
        return Ok(if !attached && ordinal < max {
            Some(OptState {
                opt,
                word: ordinal + 1,
                used,
                open: false,
                closed: false,
            })
        } else {
            None
        });
    };

    let mut buf = [0_u8; 4];
    let delim_str = delim.encode_utf8(&mut buf);
    let (segments, dangling) = split_delimited(value, delim);
    // Empty pieces are only invalid once a delimiter actually separates
    // segments; a bare empty value (`--opt=`) keeps its old tolerant behavior.
    if value.contains(delim_str) && segments.iter().any(|s| s.is_empty()) {
        return Err(());
    }
    // A closed segment must name a value the option accepts; an unknown value
    // cannot be part of a valid command line.
    if !segments_are_known(opt, &segments) {
        return Err(());
    }
    let closed = used.len() + segments.len();
    if closed > max || (closed == max && dangling) {
        return Err(());
    }
    used.extend(segments.iter().map(|s| s.to_os_string()));

    // An attached value closes the occurrence unless a dangling delimiter
    // still accepts another segment; a space-separated word keeps accepting
    // values until the range is full.
    let accepts_more = dangling || (!attached && closed < max);
    if accepts_more {
        Ok(Some(OptState {
            opt,
            word: ordinal + 1,
            used,
            open: dangling,
            closed: false,
        }))
    } else if opt.get_value_terminator().is_some() {
        // The value occurrence is complete, but an argument with a value
        // terminator still accepts the standalone terminator as the very next
        // word, no matter whether the values arrived through `=`, a separate
        // word, a short flag or an alias. Keep a one-word "closed" window so
        // every spelling reaches the same explicit end state.
        Ok(Some(OptState {
            opt,
            word: ordinal + 1,
            used,
            open: false,
            closed: true,
        }))
    } else {
        Ok(None)
    }
}

/// State after a closed `--opt=value` word: `ValueDone` when the occurrence is
/// complete, `Opt` when a dangling delimiter still accepts another segment.
/// Illegal values flag the whole command line as uncompletable.
fn attached_opt_state<'a>(
    opt: &'a clap::Arg,
    value: &OsStr,
    is_illegal: &mut bool,
) -> ParseState<'a> {
    match consume_opt_word(OptState::new(opt, 1), value, true) {
        Ok(Some(state)) => ParseState::Opt(state),
        Ok(None) => ParseState::ValueDone,
        Err(()) => {
            *is_illegal = true;
            ParseState::ValueDone
        }
    }
}

/// Advance an ongoing option occurrence by one space-separated value word.
fn advance_opt_value<'a>(
    state: OptState<'a>,
    value: &OsStr,
    is_illegal: &mut bool,
) -> ParseState<'a> {
    match consume_opt_word(state, value, false) {
        Ok(Some(next)) => ParseState::Opt(next),
        Ok(None) => ParseState::ValueDone,
        Err(()) => {
            *is_illegal = true;
            ParseState::ValueDone
        }
    }
}

/// Whether the whole shell word is exactly the given terminator.
fn word_is(terminator: &OsStr, arg: &clap_lex::ParsedArg<'_>) -> bool {
    arg.to_value_os() == terminator
}

/// Result of analyzing the value word under the cursor for an option that
/// packs delimiter-separated values.
struct DelimCursor<'s> {
    /// Text of the segment currently being typed.
    current: &'s OsStr,
    /// Closed segments that precede it, in order.
    closed: Vec<&'s OsStr>,
    /// Everything in this shell word before the current segment, including the
    /// trailing delimiter, to re-prefix candidates with.
    prefix: &'s str,
}

/// Whether every closed delimiter segment names a value the option accepts.
///
/// Options without a fixed value set accept anything.  A bare empty value
/// (`--opt=`) keeps its legacy tolerant treatment; empty segments separated
/// by a delimiter are rejected before this point.  A segment that is not
/// valid UTF-8 cannot match a possible value and is rejected, as clap's
/// parser would reject it.
fn segments_are_known(opt: &clap::Arg, segments: &[&OsStr]) -> bool {
    let Some(possible) = possible_values(opt) else {
        return true;
    };
    let ignore_case = opt.is_ignore_case_set();
    let possible: Vec<_> = possible.collect();
    segments.iter().all(|segment| {
        segment
            .to_str()
            .is_some_and(|s| s.is_empty() || possible.iter().any(|pv| pv.matches(s, ignore_case)))
    })
}

/// Split the cursor value of a bounded delimiter option into the segment being
/// edited and its already closed prefix segments.
///
/// `Ok(None)` means the option is not a bounded delimiter option (legacy
/// completion applies); `Err(())` means the cursor sits at an illegal segment
/// position (a leading/empty segment or one past the accepted number of
/// values), which must not produce candidates.
fn cursor_delimited<'s>(
    opt: &clap::Arg,
    value: &'s OsStr,
    prior_segments: usize,
) -> Result<Option<DelimCursor<'s>>, ()> {
    let Some(delim) = is_bounded_delimited(opt) else {
        return Ok(None);
    };
    let max = opt.get_num_args().expect("built").max_values();
    // The cursor word being edited keeps the legacy whole-word completion when
    // it is not valid UTF-8; earlier closed words are still tracked by bytes.
    let value_str = match value.to_str() {
        Some(s) => s,
        None => return Ok(None),
    };
    let mut buf = [0_u8; 4];
    let delim_str = delim.encode_utf8(&mut buf);

    let Some(pos) = value_str.rfind(delim) else {
        return Ok(Some(DelimCursor {
            current: value,
            closed: Vec::new(),
            prefix: "",
        }));
    };
    let (prefix, current) = value_str.split_at(pos + delim.len_utf8());
    let closed: Vec<&OsStr> = prefix[..prefix.len() - delim_str.len()]
        .split(delim)
        .map(OsStr::new)
        .collect();
    if closed.iter().any(|s| s.is_empty())
        || prior_segments + closed.len() >= max
        || !segments_are_known(opt, &closed)
    {
        return Err(());
    }
    Ok(Some(DelimCursor {
        current: OsStr::new(current),
        closed,
        prefix,
    }))
}

fn complete_arg(
    arg: &clap_lex::ParsedArg<'_>,
    cmd: &clap::Command,
    current_dir: Option<&std::path::Path>,
    pos_index: usize,
    is_escaped: bool,
    state: ParseState<'_>,
    disabled: &DisabledArgs,
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
            // A closed `--name=value` word (or a short cluster carrying an
            // inline value, e.g. `-ovalue` / `-o=value`) is the option's value
            // position: only the option's own values are completed, exactly as
            // if the value had been written as a separate word.  The content
            // after `=` never takes part in subcommand, positional or
            // option-name completion, and the word itself is not yet part of
            // the conflict set.
            if !is_escaped {
                if let Some(values) = complete_inline_option_value(arg, cmd, current_dir, disabled)
                {
                    let values =
                        values.map_err(|()| std::io::Error::other("no completion generated"))?;
                    completions.extend(values);
                    return finalize_completions(completions);
                }
            }

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
                if !disabled.hidden.contains(positional.get_id()) {
                    completions.extend(complete_arg_value(
                        arg.to_value(),
                        positional,
                        current_dir,
                        0,
                    ));
                }
            }
            if !is_escaped {
                completions.extend(complete_option(arg, cmd, current_dir, disabled)?);
            }
        }
        ParseState::Pos((_, num_arg)) => {
            if let Some(positional) = cmd
                .get_positionals()
                .find(|p| p.get_index() == Some(pos_index))
            {
                if !disabled.hidden.contains(positional.get_id()) {
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
                    completions.extend(complete_option(arg, cmd, current_dir, disabled)?);
                }
            }
        }
        ParseState::Opt(state) => {
            let opt = state.opt;
            // A value terminator is only ever its own whole word for a bounded
            // delimited option.
            if let Some(terminator) = opt.get_value_terminator() {
                let min = opt.get_num_args().expect("built").min_values();
                let consumed_words = state.word.saturating_sub(1);
                let word = arg.to_value_os();
                let term = OsStr::new(terminator.as_str());
                let is_exact = word == term;
                let is_glued = !is_exact && word.contains(terminator.as_str());
                if is_glued && glued_terminator_is_illegal(opt) {
                    // A terminator glued onto a segment of a bounded, fixed-set
                    // delimiter option cannot be a value; the engine never
                    // guesses a split into value plus terminator. A free-form
                    // option takes the whole word instead.
                    return Err(std::io::Error::other("no completion generated"));
                }
                if is_exact {
                    if state.open || consumed_words < min {
                        // A dangling delimiter still owes its segment and the
                        // option still owes its minimum number of value words.
                        return Err(std::io::Error::other("no completion generated"));
                    }
                    // The terminator closes value taking and is never itself a
                    // candidate.
                    return finalize_completions(completions);
                }
                if state.closed {
                    // The value occurrence already closed (`--opt=value` or a
                    // full delimiter set); only the terminator word still
                    // belongs to it. Complete ordinary options/positionals.
                    return complete_arg(
                        arg,
                        cmd,
                        current_dir,
                        pos_index,
                        is_escaped,
                        ParseState::ValueDone,
                        disabled,
                    );
                }
            }
            // Offer this waiting option's own values. It is normally offered
            // even when its own (recorded) spelling put it in the hidden set,
            // but not when a *different* option already present conflicts with
            // it: then clap rejects the line regardless and its values stay
            // hidden, preserving the established conflict behavior.
            if !disabled.hidden.contains(opt.get_id())
                || !disabled.hidden_by_present_conflict(cmd, opt)
            {
                match complete_separate_opt_value(arg.to_value(), &state, current_dir) {
                    Ok(values) => completions.extend(values),
                    // The cursor sits at an illegal segment position (an empty
                    // segment or one past the accepted number of values).
                    Err(()) => return Err(std::io::Error::other("no completion generated")),
                }
            }
            let min = opt.get_num_args().map(|r| r.min_values()).unwrap_or(0);
            if state.word > min && !state.open {
                // Also complete this raw_arg as a positional argument, flags,
                // options and subcommand.  A dangling delimiter (`state.open`)
                // requires the next word to be this option's segment: it cannot
                // be a valid stop, so nothing else is offered there.
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

    finalize_completions(completions)
}

/// Hide hidden candidates when visible ones exist, deduplicate by id and sort
/// into the presentation order shared by every completion source.
fn finalize_completions(
    mut completions: Vec<CompletionCandidate>,
) -> Result<Vec<CompletionCandidate>, std::io::Error> {
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
    // Different sources (a subcommand and a positional value, an option alias
    // and a directory entry, ...) can propose the same literal insertion text
    // under different ids; only the first is kept.  Retaining by first
    // occurrence keeps the presentation order untouched by aliases or
    // duplicate directory entries.
    let mut seen_values = HashSet::new();
    completions.retain(move |a| seen_values.insert(a.get_value().to_os_string()));

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

/// Generate value candidates for the segment under the cursor of a
/// delimiter-separated multi-value option.
///
/// `prior_segments` is the number of segments already closed by earlier shell
/// words; `prior_used` are their literal values, suppressed from the result so
/// an already selected value is not offered a second time.  Candidates cover
/// only the current shell word, so `prefix` from closed segments is limited to
/// the current word as well.
///
/// `Ok` carries the candidates; `Err(())` marks an illegal segment position
/// (an empty segment or a segment past the accepted number of values).
fn complete_segment_value(
    value: &OsStr,
    opt: &clap::Arg,
    prior_segments: usize,
    prior_used: &[&OsStr],
    arg_index: usize,
    current_dir: Option<&std::path::Path>,
) -> Result<Vec<CompletionCandidate>, ()> {
    // For a bounded delimiter option with a fixed value set, the terminator is
    // recognized only as a standalone word; glued onto a segment (`--opt=a;`,
    // `--opt=;`) it cannot be part of a valid line and the engine never guesses
    // a split. A free-form option takes the whole word, like clap's parser.
    if glued_terminator_is_illegal(opt) {
        if let Some(terminator) = opt.get_value_terminator() {
            if value.contains(terminator.as_str()) {
                return Err(());
            }
        }
    }
    // Text glued onto a complete possible value (`3v`, `-3v`) cannot be a
    // value nor a following short cluster; the word is illegal as a whole.
    if glued_fixed_value_is_illegal(opt, value) {
        return Err(());
    }
    let Some(cursor) = cursor_delimited(opt, value, prior_segments)? else {
        // Not a bounded delimiter option: keep the legacy whole-word
        // completion (its own `rsplit_delimiter` handles single-value args).
        return Ok(complete_arg_value(
            value.to_str().ok_or(value),
            opt,
            current_dir,
            arg_index,
        ));
    };

    let mut used: Vec<&OsStr> = prior_used.to_vec();
    used.extend(cursor.closed.iter().copied());

    let mut values = complete_arg_value(
        Ok(cursor.current.to_str().ok_or(())?),
        opt,
        current_dir,
        arg_index,
    );
    // An already closed segment stays selected and must not reappear.
    values.retain(|comp| !used.iter().any(|u| *u == comp.get_value()));
    if !cursor.prefix.is_empty() {
        values = values
            .into_iter()
            .map(|comp| comp.add_prefix(cursor.prefix))
            .collect();
    }
    Ok(values)
}

/// Completion at a space-separated option value word tracked by `state`.
fn complete_separate_opt_value(
    value: Result<&str, &OsStr>,
    state: &OptState<'_>,
    current_dir: Option<&std::path::Path>,
) -> Result<Vec<CompletionCandidate>, ()> {
    let value_os = match value {
        Ok(value) => OsStr::new(value),
        Err(value_os) => value_os,
    };
    let used: Vec<&OsStr> = state.used.iter().map(OsString::as_os_str).collect();
    complete_segment_value(
        value_os,
        state.opt,
        state.used.len(),
        &used,
        state.word.saturating_sub(1),
        current_dir,
    )
}

/// Complete the inline value carried by the cursor's option word.
///
/// This covers `--name=value` as well as short clusters with an attached
/// value, e.g. `-ovalue` and `-o=value` (including leading flags, `-abovalue`).
/// It returns the option's value candidates, re-prefixed with the exact
/// option spelling, when the cursor is inside such a value.  The whole word is
/// consumed by its option: nothing after `=` or past the taking flag takes
/// part in option-name, subcommand or positional completion, and the word is
/// not yet part of the conflict set.
///
/// A bare `--name`/`-o` whose value is still a separate word completes
/// normally, as does an unknown long name, a recognized option that takes no
/// value and anything else that cannot be recognized: `None` preserves the
/// existing failure semantics rather than guessing a split.
///
/// `Some(Err(()))` means the recognized word is an illegal value position for
/// a bounded delimiter option (an empty segment or a segment past the accepted
/// number of values) and must yield "no completion generated".
fn complete_inline_option_value(
    arg: &clap_lex::ParsedArg<'_>,
    cmd: &clap::Command,
    current_dir: Option<&std::path::Path>,
    disabled: &DisabledArgs,
) -> Option<Result<Vec<CompletionCandidate>, ()>> {
    if let Some((flag, value)) = arg.to_long() {
        let flag = flag.ok()?;
        // A bare `--name` has no attached value and is completed as an option name.
        let value = value?;
        let opt = cmd.get_arguments().find(|a| {
            a.get_long_and_visible_aliases()
                .is_some_and(|longs| longs.into_iter().any(|long| long == flag))
        })?;
        if !opt.get_num_args().expect("built").takes_values() {
            return None;
        }
        if disabled.hidden.contains(opt.get_id()) {
            return Some(Ok(Vec::new()));
        }
        let prefix = format!("--{flag}=");
        return Some(
            complete_segment_value(value, opt, 0, &[], 0, current_dir).map(|values| {
                values
                    .into_iter()
                    .map(|comp| comp.add_prefix(&prefix))
                    .collect()
            }),
        );
    }

    let short = arg.to_short()?;
    if short.is_negative_number() {
        return None;
    }
    let cluster = scan_short_cluster(cmd, short);
    let opt = cluster.takes_value_opt?;

    // Detect an attached value: once the cluster reaches its value-taking
    // flag, everything left is the value, optionally introduced by `=`.  With
    // no remainder and no `=` (`-o`), the word is left to the regular option
    // completion, which already offers inline values.
    let mut value_flags = cluster.value_flags.clone();
    let mut peek = value_flags.clone();
    let has_equal = matches!(peek.next_flag(), Some(Ok('=')));
    if !has_equal && value_flags.is_empty() {
        return None;
    }
    // An unknown or repeated flag before the taking flag leaves the word to
    // regular option completion; such a cluster cannot carry this option's
    // value.
    if !cluster.flags_valid || cluster.duplicate {
        return None;
    }
    if has_equal {
        // Consume the `=` so the remainder is the bare value (possibly empty
        // for `-o=`), matching the space-separated empty-value position.
        value_flags.next_flag();
    }
    if disabled.hidden.contains(opt.get_id()) {
        return Some(Ok(Vec::new()));
    }
    let value = value_flags.next_value_os().unwrap_or(OsStr::new(""));
    let sep = if has_equal { "=" } else { "" };
    let prefix = format!("-{}{sep}", cluster.leading_flags);
    Some(
        complete_segment_value(value, opt, 0, &[], 0, current_dir).map(|values| {
            values
                .into_iter()
                .map(|comp| comp.add_prefix(&prefix))
                .collect()
        }),
    )
}

fn complete_option(
    arg: &clap_lex::ParsedArg<'_>,
    cmd: &clap::Command,
    current_dir: Option<&std::path::Path>,
    disabled: &DisabledArgs,
) -> Result<Vec<CompletionCandidate>, std::io::Error> {
    debug!("complete_option: arg={arg:?}, current_dir={current_dir:?}");
    let mut completions = Vec::<CompletionCandidate>::new();
    if arg.is_empty() {
        completions.extend(longs_and_visible_aliases(cmd, &disabled.hidden));
        completions.extend(hidden_longs_aliases(cmd, &disabled.hidden));

        let dash_or_arg = if arg.is_empty() {
            "-".into()
        } else {
            arg.to_value_os().to_string_lossy()
        };
        completions.extend(
            shorts_and_visible_aliases(cmd, &disabled.hidden)
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
            shorts_and_visible_aliases(cmd, &disabled.hidden)
                .into_iter()
                .map(|comp| comp.add_prefix(dash_or_arg.to_string())),
        );

        completions.extend(longs_and_visible_aliases(cmd, &disabled.hidden));
        completions.extend(hidden_longs_aliases(cmd, &disabled.hidden));
    } else if arg.is_escape() {
        // HACK: Assuming knowledge of is_escape
        completions.extend(longs_and_visible_aliases(cmd, &disabled.hidden));
        completions.extend(hidden_longs_aliases(cmd, &disabled.hidden));
    } else if let Some((flag, value)) = arg.to_long() {
        if let Ok(flag) = flag {
            if let Some(value) = value {
                let opt = cmd.get_arguments().find(|a| {
                    a.get_long_and_visible_aliases()
                        .is_some_and(|longs| longs.into_iter().any(|long| long == flag))
                });
                if let Some(arg) = opt {
                    if !disabled.hidden.contains(arg.get_id()) {
                        match complete_segment_value(value, arg, 0, &[], 0, current_dir) {
                            Ok(values) => {
                                completions.extend(
                                    values
                                        .into_iter()
                                        .map(|comp| comp.add_prefix(format!("--{flag}="))),
                                );
                            }
                            // An illegal mixed word (text glued onto a
                            // complete value) completes to nothing.
                            Err(()) => {
                                return Err(std::io::Error::other("no completion generated"));
                            }
                        }
                    }
                }
            } else {
                completions.extend(
                    longs_and_visible_aliases(cmd, &disabled.hidden)
                        .into_iter()
                        .filter(|comp| comp.get_value().starts_with(format!("--{flag}").as_str())),
                );
                completions.extend(
                    hidden_longs_aliases(cmd, &disabled.hidden)
                        .into_iter()
                        .filter(|comp| comp.get_value().starts_with(format!("--{flag}").as_str())),
                );
            }
        }
    } else if let Some(short) = arg.to_short() {
        if !short.is_negative_number() {
            let cluster = scan_short_cluster(cmd, short);
            // A cluster carrying an unknown member or repeating a
            // non-repeatable flag is illegal at the cursor too: it is never a
            // prefix of a valid command line, so completion errors rather than
            // offering flags that could be appended to an invalid word.
            if !cluster.flags_valid || cluster.duplicate {
                return Err(std::io::Error::other("no completion generated"));
            }

            if let Some(opt) = cluster.takes_value_opt {
                if !disabled.hidden.contains(opt.get_id()) {
                    let mut value_flags = cluster.value_flags.clone();
                    let mut peek_short = value_flags.clone();
                    let has_equal = if let Some(Ok('=')) = peek_short.next_flag() {
                        value_flags.next_flag();
                        true
                    } else {
                        false
                    };

                    let value = value_flags.next_value_os().unwrap_or(OsStr::new(""));
                    match complete_segment_value(value, opt, 0, &[], 0, current_dir) {
                        Ok(values) => {
                            completions.extend(values.into_iter().map(|comp| {
                                let sep = if has_equal { "=" } else { "" };
                                comp.add_prefix(format!("-{}{sep}", cluster.leading_flags))
                            }));
                        }
                        // A value with short letters glued onto a complete
                        // possible value is an illegal mixed word, never a
                        // partial cluster.
                        Err(()) => return Err(std::io::Error::other("no completion generated")),
                    }
                }
            } else {
                // Offer another flag to append to the cluster. A flag already
                // written in this cluster is offered again only when it is
                // repeatable (`Count`/`Append`); appending it otherwise would
                // build an illegal repeated cluster such as `-vv`.
                let written: Vec<char> = cluster.leading_flags.chars().collect();
                completions.extend(
                    shorts_and_visible_aliases(cmd, &disabled.hidden)
                        .into_iter()
                        .filter(|comp| {
                            let ch = comp
                                .get_value()
                                .to_str()
                                .and_then(|s| s.chars().next())
                                .unwrap_or_default();
                            if let Some(arg) = find_short_arg(cmd, ch) {
                                if !arg_is_repeatable(arg) && written.contains(&ch) {
                                    return false;
                                }
                            }
                            true
                        })
                        .map(|comp| comp.add_prefix(format!("-{}", cluster.leading_flags))),
                );
            }
        }
    }
    debug!("complete_option: completions={completions:?}");
    Ok(completions)
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

/// Resolve a closed word to the subcommand it selects on this level.
///
/// This mirrors clap's parser: an exact name or alias always wins (so a
/// positional value that merely shares a prefix, e.g. `remote`, cannot shadow a
/// subcommand of the same name); otherwise, when subcommand inference is
/// enabled, a unique name-or-alias prefix selects that subcommand. A prefix
/// shared by several subcommands resolves to nothing here, just as clap
/// rejects such a word rather than choosing a branch; the caller then treats
/// it as an ordinary positional value.
fn select_subcommand<'c>(cmd: &'c clap::Command, value: &str) -> Option<&'c clap::Command> {
    if let Some(subcommand) = cmd.find_subcommand(value) {
        return Some(subcommand);
    }
    if cmd.is_infer_subcommands_set() {
        let mut matches = cmd.get_subcommands().filter(|subcommand| {
            subcommand.get_name().starts_with(value)
                || subcommand
                    .get_all_aliases()
                    .any(|alias| alias.starts_with(value))
        });
        let first = matches.next()?;
        if matches.next().is_none() {
            return Some(first);
        }
    }
    None
}

/// Whether a closed (already submitted) word can occupy the positional slot at
/// `pos_index`, mirroring clap's parser.
///
/// A positional without a fixed value set (a free-form parser, a path hint or
/// a custom completer) accepts every word. A positional backed by possible
/// values accepts only one of them (by name or alias, honoring
/// `ignore_case`), splitting a bounded delimiter word into its segments just
/// like an option value. With no positional slot left, an external subcommand
/// command swallows the word; otherwise clap rejects the line.
fn closed_positional_is_accepted(cmd: &clap::Command, pos_index: usize, word: &OsStr) -> bool {
    let Some(positional) = cmd
        .get_positionals()
        .find(|p| p.get_index() == Some(pos_index))
    else {
        // No slot remains: an external subcommand accepts any trailing word,
        // anything else is an unrecognized subcommand/argument.
        return cmd.is_allow_external_subcommands_set();
    };

    let Some(possible) = possible_values(positional) else {
        return true;
    };
    let ignore_case = positional.is_ignore_case_set();
    let possible: Vec<_> = possible.collect();

    let segments: Vec<&OsStr> = match is_bounded_delimited(positional) {
        Some(delim) => {
            let (segments, _dangling) = split_delimited(word, delim);
            segments
        }
        None => vec![word],
    };
    segments.iter().all(|segment| {
        segment
            .to_str()
            .is_some_and(|s| possible.iter().any(|pv| pv.matches(s, ignore_case)))
    })
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

    // `subcommands` lists the canonical name before each of its aliases; drop
    // aliases of a subcommand whose canonical name is still offered. This must
    // happen before the alphabetical sort, or a short alias (`r` for `run`,
    // `rem` for `remote`) would sort ahead of the canonical name and survive
    // the later by-id de-duplication instead of it.
    let mut seen_ids = HashSet::new();
    scs.retain(|candidate| {
        candidate
            .get_id()
            .cloned()
            .map(|id| seen_ids.insert(id))
            .unwrap_or(true)
    });

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

/// Result of scanning a short cluster up to its first value-taking option.
#[derive(Debug)]
struct ShortCluster<'c, 's> {
    /// Flag characters scanned before the value-taking option, in order.
    leading_flags: String,
    /// Arguments those leading flags resolve to. `None` marks a character that
    /// names no option, so the entries line up with `leading_flags`.
    leading_args: Vec<Option<&'c clap::Arg>>,
    /// The first option that takes a value, if the cluster reached one.
    takes_value_opt: Option<&'c clap::Arg>,
    /// Iterator positioned right after the taking flag, exposing any attached
    /// value (`-ovalue` / `-o=value`).
    value_flags: clap_lex::ShortFlags<'s>,
    /// Every scanned member was a recognized option (or the taking option);
    /// an unknown character or invalid UTF-8 makes this `false`.
    flags_valid: bool,
    /// A non-repeatable leading flag occurs more than once in the cluster
    /// (e.g. `-vv` for a `SetTrue` flag).
    duplicate: bool,
}

/// Scan the short flags and find the first `takes_values` option.
///
/// Anything after the first value-taking option is that option's attached
/// value and therefore not scanned. A repeatable flag (`Count`/`Append`, e.g.
/// `-cc`) is never reported as a duplicate.
fn scan_short_cluster<'c, 's>(
    cmd: &'c clap::Command,
    mut short: clap_lex::ShortFlags<'s>,
) -> ShortCluster<'c, 's> {
    let mut leading_flags = String::new();
    let mut leading_args: Vec<Option<&clap::Arg>> = Vec::new();
    let mut flags_valid = true;
    let mut duplicate = false;
    loop {
        match short.next_flag() {
            Some(Ok(flag)) => {
                let found = find_short_arg(cmd, flag);
                if let Some(arg) = found {
                    if arg.get_num_args().expect("built").takes_values() {
                        // The taking flag stays part of the written prefix
                        // (`-cS...`), but it is tracked separately from the
                        // leading value-less flags.
                        leading_flags.push(flag);
                        return ShortCluster {
                            leading_flags,
                            leading_args,
                            takes_value_opt: Some(arg),
                            value_flags: short,
                            flags_valid,
                            duplicate,
                        };
                    }
                    if !arg_is_repeatable(arg)
                        && leading_args
                            .iter()
                            .any(|earlier| earlier.is_some_and(|a| a.get_id() == arg.get_id()))
                    {
                        duplicate = true;
                    }
                } else {
                    flags_valid = false;
                }
                leading_flags.push(flag);
                leading_args.push(found);
            }
            Some(Err(_)) => {
                flags_valid = false;
                break;
            }
            None => break,
        }
    }

    ShortCluster {
        leading_flags,
        leading_args,
        takes_value_opt: None,
        value_flags: short,
        flags_valid,
        duplicate,
    }
}

/// Resolve a short flag character (or visible short alias) to its argument.
fn find_short_arg(cmd: &clap::Command, flag: char) -> Option<&clap::Arg> {
    cmd.get_arguments().find(|a| {
        a.get_short_and_visible_aliases()
            .is_some_and(|shorts| shorts.contains(&flag))
    })
}

/// Whether an option may occur more than once on one command line.
///
/// `Count` and `Append` accept repeated occurrences; every other action
/// conflicts with itself when clap parses it a second time.
fn arg_is_repeatable(arg: &clap::Arg) -> bool {
    matches!(
        arg.get_action(),
        clap::ArgAction::Count | clap::ArgAction::Append
    )
}

/// Whether a closed value word names one of an option's fixed possible values.
///
/// Options without a fixed value set accept every word. This only covers
/// options without a value delimiter; delimiter-separated values keep their
/// legacy segment handling.
fn closed_word_value_is_known(opt: &clap::Arg, value: &OsStr) -> bool {
    if opt.get_value_delimiter().is_some() {
        return true;
    }
    let Some(mut possible) = possible_values(opt) else {
        return true;
    };
    let Some(value) = value.to_str() else {
        return false;
    };
    let ignore_case = opt.is_ignore_case_set();
    possible.any(|pv| pv.matches(value, ignore_case))
}

/// Whether the value under the cursor glues extra short-flag-looking text onto
/// a complete fixed possible value, e.g. `3v` or `-3v` when `3`/`-3` are
/// already complete values. clap rejects such a word rather than splitting the
/// value from more flags, and completion must not read the trailing letters as a
/// new cluster or succeed partially. A plain prefix with no match (`9`) or a
/// numeric extension (`30`) still completes as an empty candidate list; only a
/// glued letter marks an illegal mixed word.
fn glued_fixed_value_is_illegal(opt: &clap::Arg, value: &OsStr) -> bool {
    if opt.get_value_delimiter().is_some() {
        return false;
    }
    let Some(possible) = possible_values(opt) else {
        return false;
    };
    let Some(value) = value.to_str() else {
        return false;
    };
    if value.is_empty() {
        return false;
    }
    let ignore_case = opt.is_ignore_case_set();
    let possible: Vec<_> = possible.collect();
    if possible.iter().any(|pv| pv.matches(value, ignore_case)) {
        // Still editing this exact value.
        return false;
    }
    possible.iter().any(|pv| {
        let name = pv.get_name();
        match value.strip_prefix(name) {
            Some(rest) => !rest.is_empty() && rest.starts_with(char::is_alphabetic),
            None => false,
        }
    })
}

/// Whether the waiting option accepts a negative-number-looking word as its
/// value: either it explicitly allows negative numbers or one of its fixed
/// possible values is the word itself.
fn opt_accepts_negative_value(opt: &clap::Arg, word: &OsStr) -> bool {
    if opt.is_allow_negative_numbers_set() {
        return true;
    }
    closed_word_value_is_known(opt, word)
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

fn pos_allows_hyphen(cmd: &clap::Command, pos_index: usize) -> bool {
    cmd.get_positionals()
        .find(|a| a.get_index() == Some(pos_index))
        .map(|p| p.is_allow_hyphen_values_set())
        .unwrap_or(false)
}

fn pos_allows_negative(cmd: &clap::Command, pos_index: usize) -> bool {
    cmd.get_positionals()
        .find(|a| a.get_index() == Some(pos_index))
        .map(|p| p.is_allow_negative_numbers_set())
        .unwrap_or(false)
}

fn opt_allows_hyphen(state: &ParseState<'_>, arg: &clap_lex::ParsedArg<'_>) -> bool {
    let val = arg.to_value_os();
    if val.starts_with("-") {
        if let ParseState::Opt(state) = state {
            return state.opt.is_allow_hyphen_values_set();
        }
    }

    false
}

/// Classification of a closed word while an option occurrence may take values.
#[derive(Debug, PartialEq, Eq)]
enum ClosedValueWord {
    /// The word is consumed as the option's value.
    Value,
    /// The word cannot belong to the command line (missing value, unknown
    /// value, illegal option word while a segment is mandatory).
    Illegal,
    /// The occurrence may stop here and the word resumes ordinary parsing.
    ResumeParsing,
}

/// Classify a closed (already submitted) word while `state`'s option may still
/// take value words, mirroring clap's parser:
///
/// * an option with `allow_hyphen_values` takes every word;
/// * while a dangling delimiter segment is mandatory (`state.open`), every
///   option-shaped word or `--` is illegal;
/// * while the minimum number of value words is still owed, every word is the
///   value, so an option word means a missing value and an unknown value is
///   rejected;
/// * once the minimum is met, an option word or `--` may stop the occurrence
///   and resumes ordinary parsing, while a plain value word is validated and
///   consumed up to the accepted number of values.
fn classify_closed_value_word(
    state: &OptState<'_>,
    arg: &clap_lex::ParsedArg<'_>,
) -> ClosedValueWord {
    let opt = state.opt;
    let range = opt.get_num_args().expect("built");
    let min = range.min_values();
    let max = range.max_values();
    let consumed_words = state.word.saturating_sub(1);
    let word = arg.to_value_os();

    if opt.is_allow_hyphen_values_set() {
        return ClosedValueWord::Value;
    }

    let is_option_word = arg.is_escape()
        || arg.to_long().is_some()
        || arg
            .to_short()
            .is_some_and(|short| !short.is_negative_number());
    // A negative-number-shaped word is the option's value when the option
    // accepts negative numbers or one of its fixed values matches the word.
    let negative_as_value = arg
        .to_short()
        .is_some_and(|short| short.is_negative_number() && opt_accepts_negative_value(opt, word));

    if state.open {
        // A dangling delimiter still demands its segment.
        if is_option_word && !negative_as_value {
            ClosedValueWord::Illegal
        } else {
            ClosedValueWord::Value
        }
    } else if is_option_word && !negative_as_value {
        // The occurrence still owes at least one value word: the word cannot
        // both close the occurrence and name another option, so clap reports a
        // missing value. Once the minimum is met, an option word is a legal
        // stop and ordinary parsing resumes.
        if consumed_words < min {
            ClosedValueWord::Illegal
        } else {
            ClosedValueWord::ResumeParsing
        }
    } else if !closed_word_value_is_known(opt, word) {
        // A plain word that is not one of the option's fixed values makes the
        // line invalid; only a fixed value set rejects it.
        ClosedValueWord::Illegal
    } else if consumed_words >= max {
        // The occurrence is full; the extra word belongs to ordinary parsing.
        ClosedValueWord::ResumeParsing
    } else {
        ClosedValueWord::Value
    }
}

/// Record the explicitly supplied flags of a structurally valid closed
/// cluster and flag a repeated non-repeatable option.
fn handle_closed_cluster(
    cluster: &ShortCluster<'_, '_>,
    explicit: &mut HashSet<Id>,
    is_illegal: &mut bool,
) {
    for arg in cluster.leading_args.iter().flatten() {
        if !arg_is_repeatable(arg) && explicit.contains(arg.get_id()) {
            *is_illegal = true;
        }
        explicit.insert(arg.get_id().clone());
    }
    if let Some(opt) = cluster.takes_value_opt {
        if !arg_is_repeatable(opt) && explicit.contains(opt.get_id()) {
            *is_illegal = true;
        }
        explicit.insert(opt.get_id().clone());
    }
}

/// Arguments hidden from completion at the cursor.
#[derive(Debug, Default)]
struct DisabledArgs {
    /// Ids hidden from option-name and positional listings: options that
    /// already occurred (and cannot repeat) plus every option they conflict
    /// with.
    hidden: HashSet<Id>,
    /// Ids of options explicitly present in the words before the cursor. Used
    /// to tell a self-inflicted "already present" hiding from a genuine
    /// conflict with another present option.
    present: HashSet<Id>,
}

impl DisabledArgs {
    /// Whether `opt` is hidden because a *different* option already present on
    /// the line conflicts with it. Its own presence alone is not a conflict.
    fn hidden_by_present_conflict(&self, cmd: &clap::Command, opt: &clap::Arg) -> bool {
        arg_direct_conflicts(cmd, opt)
            .iter()
            .any(|id| id != opt.get_id() && self.present.contains(id))
    }
}

/// Arguments that must not be offered because they conflict with an explicitly
/// present option or because they already occurred and cannot occur a second
/// time (every action except `Count`/`Append`). Repeatable options stay
/// available.
fn gather_disabled_args(cmd: &clap::Command, present: &HashSet<Id>) -> HashSet<Id> {
    let mut disabled = HashSet::new();
    if present.is_empty() {
        return disabled;
    }
    for arg in cmd.get_arguments() {
        if present.contains(arg.get_id()) {
            if !arg_is_repeatable(arg) {
                // The option was already consumed; clap rejects a second
                // occurrence, so its name is not offered again.
                disabled.insert(arg.get_id().clone());
            }
            // A present option hides everything it conflicts with ...
            for id in arg_direct_conflicts(cmd, arg) {
                if !present.contains(&id) {
                    disabled.insert(id);
                }
            }
        } else {
            // ... and is hidden when either side declares the conflict.
            if arg_direct_conflicts(cmd, arg)
                .iter()
                .any(|id| present.contains(id))
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
