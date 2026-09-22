use std::io::{Error, Write};

use clap::{Arg, Command, ValueHint, builder};

use crate::generator::{Generator, utils};

/// Generate fish completion file
///
/// Note: The fish generator currently only supports named options (-o/--option), not positional arguments.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct Fish;

impl Generator for Fish {
    fn file_name(&self, name: &str) -> String {
        format!("{name}.fish")
    }

    fn generate(&self, cmd: &Command, buf: &mut dyn Write) {
        self.try_generate(cmd, buf)
            .expect("failed to write completion file");
    }

    fn try_generate(&self, cmd: &Command, buf: &mut dyn Write) -> Result<(), Error> {
        let bin_name = cmd
            .get_bin_name()
            .expect("crate::generate should have set the bin_name");

        let name = escape_name(bin_name);
        let mut needs_fn_name = &format!("__fish_{name}_needs_command")[..];
        let mut using_fn_name = &format!("__fish_{name}_using_subcommand")[..];
        // Given `git --git-dir somedir status`, using `__fish_seen_subcommand_from` won't help us
        // find out `status` is the real subcommand, and not `somedir`. However, when there are no subcommands,
        // there is no need to use our custom stubs.
        if cmd.has_subcommands() {
            gen_subcommand_helpers(&name, cmd, buf, needs_fn_name, using_fn_name);
        } else {
            needs_fn_name = "__fish_use_subcommand";
            using_fn_name = "__fish_seen_subcommand_from";
        }

        let generator = FishGen {
            root_command: bin_name,
            needs_fn_name,
            using_fn_name,
            helpers: String::new(),
            lines: String::new(),
        };
        let generator = generator.run(cmd);
        write!(buf, "{}{}", generator.helpers, generator.lines)
    }
}

/// Mutable state carried through the recursive completion generation.
struct FishGen<'a> {
    root_command: &'a str,
    needs_fn_name: &'a str,
    using_fn_name: &'a str,
    /// Predicate functions emitted ahead of the `complete` lines.
    helpers: String,
    /// The `complete` lines.
    lines: String,
}

impl<'a> FishGen<'a> {
    fn run(mut self, root: &'a Command) -> Self {
        // Seed the chain with the root command so its options are stripped too (e.g. a root
        // `--config <file>` before the subcommand path).  The root link carries option specs but
        // no positional name: it is not itself matched against the subcommand tokens.
        let root_chain = [ChainCommand {
            name: None,
            optspecs: command_optspecs(root),
        }];
        let root_value_options = command_value_option_tokens(root);
        self.gen_cmd(root, &[], &root_chain, &root_value_options);
        self
    }

    /// Generate completions for `cmd` and recurse into its subcommands.
    ///
    /// `parent_commands` holds the visible subcommand names on the path to `cmd`; `chain`
    /// mirrors them (plus per-command argparse optspecs) and `value_options` accumulates the
    /// value-taking option tokens used to pad the token currently being edited.
    fn gen_cmd(
        &mut self,
        cmd: &Command,
        parent_commands: &[&str],
        chain: &[ChainCommand],
        value_options: &[String],
    ) {
        let root_command = self.root_command;
        let needs_fn_name = self.needs_fn_name;
        let using_fn_name = self.using_fn_name;
        debug!("gen_cmd");
        // example :
        //
        // complete
        //      -c {command}
        //      -d "{description}"
        //      -s {short}
        //      -l {long}
        //      -a "{possible_arguments}"
        //      -r # if require parameter
        //      -f # don't use file completion
        //      -n "{needs_fn_name}"            # complete for command "myprog"
        //      -n "{using_fn_name} subcmd1"    # complete for command "myprog subcmd1"

        let mut basic_template = format!("complete -c {root_command}");

        let is_deep = parent_commands.len() >= 3;
        if parent_commands.is_empty() {
            if cmd.has_subcommands() {
                basic_template.push_str(&format!(" -n \"{needs_fn_name}\""));
            }
        } else if !is_deep {
            // Shallow subcommands (one or two levels deep) use fish's stock
            // `__fish_seen_subcommand_from`-based predicates, preserving the historical output.
            let mut out = String::from(using_fn_name);
            match parent_commands {
                [] => unreachable!(),
                [command] => {
                    out.push_str(&format!(" {command}"));
                    if cmd.has_subcommands() {
                        out.push_str("; and not __fish_seen_subcommand_from");
                    }
                    let subcommands = cmd
                        .get_subcommands()
                        .flat_map(Command::get_name_and_visible_aliases);
                    for name in subcommands {
                        out.push_str(&format!(" {name}"));
                    }
                }
                [command, subcommand] => out.push_str(&format!(
                    " {command}; and __fish_seen_subcommand_from {subcommand}"
                )),
                _ => unreachable!(),
            }
            basic_template.push_str(format!(" -n \"{out}\"").as_str());
        } else {
            // Arbitrarily nested subcommands cannot be expressed with
            // `__fish_seen_subcommand_from` (which only looks at a single token), so each
            // completion references a dedicated predicate matching the *full, ordered* command
            // chain.  This keeps two branches that reuse identical subcommand names from
            // cross-wiring their completions.  The predicate itself is emitted lazily (see below)
            // only when this command actually contributes a completion.
            let helper_name = chain_helper_name(root_command, chain);
            basic_template.push_str(&format!(" -n \"{helper_name}\""));
        }

        debug!("gen_cmd: parent_commands={parent_commands:?}");

        // Buffer this command's own completion lines.  For deep nodes this lets us define the chain
        // predicate only when the command actually contributes a completion (the auto-generated
        // `help` stubs can contribute none); shallow nodes always flush them.
        let mut node_lines = String::new();
        let out: &mut String = &mut node_lines;

        for option in cmd.get_opts() {
            let mut template = basic_template.clone();

            if let Some(shorts) = option.get_short_and_visible_aliases() {
                for short in shorts {
                    template.push_str(format!(" -s {short}").as_str());
                }
            }

            if let Some(longs) = option.get_long_and_visible_aliases() {
                for long in longs {
                    template.push_str(format!(" -l {}", escape_string(long, false)).as_str());
                }
            }

            if let Some(data) = option.get_help() {
                template.push_str(&format!(" -d '{}'", escape_help(data)));
            }

            template.push_str(value_completion(option).as_str());

            out.push_str(template.as_str());
            out.push('\n');
        }

        for flag in utils::flags(cmd) {
            let mut template = basic_template.clone();

            if let Some(shorts) = flag.get_short_and_visible_aliases() {
                for short in shorts {
                    template.push_str(format!(" -s {short}").as_str());
                }
            }

            if let Some(longs) = flag.get_long_and_visible_aliases() {
                for long in longs {
                    template.push_str(format!(" -l {}", escape_string(long, false)).as_str());
                }
            }

            if let Some(data) = flag.get_help() {
                template.push_str(&format!(" -d '{}'", escape_help(data)));
            }

            out.push_str(template.as_str());
            out.push('\n');
        }

        let has_positionals = cmd.get_positionals().next().is_some();
        if !has_positionals {
            basic_template.push_str(" -f");
        }
        for subcommand in cmd.get_subcommands() {
            for subcommand_name in subcommand.get_name_and_visible_aliases() {
                let mut template = basic_template.clone();

                template.push_str(format!(" -a \"{subcommand_name}\"").as_str());

                if let Some(data) = subcommand.get_about() {
                    template.push_str(format!(" -d '{}'", escape_help(data)).as_str());
                }

                out.push_str(template.as_str());
                out.push('\n');
            }
        }

        if is_deep {
            if !node_lines.is_empty() {
                write_chain_helper(&mut self.helpers, root_command, chain, value_options, cmd);
                self.lines.push_str(&node_lines);
            }
        } else {
            self.lines.push_str(&node_lines);
        }

        // generate options of subcommands
        for subcommand in cmd.get_subcommands() {
            for subcommand_name in subcommand.get_name_and_visible_aliases() {
                let mut parent_commands: Vec<_> = parent_commands.into();
                parent_commands.push(subcommand_name);

                // Extend the ordered command chain with this subcommand (once per visible alias),
                // carrying its options so deep chain predicates can strip them before matching.
                let mut child_chain: Vec<ChainCommand> = chain.to_vec();
                child_chain.push(ChainCommand {
                    name: Some(subcommand_name.to_owned()),
                    optspecs: command_optspecs(subcommand),
                });
                let mut child_value_options = value_options.to_vec();
                child_value_options.extend(command_value_option_tokens(subcommand));

                self.gen_cmd(
                    subcommand,
                    &parent_commands,
                    &child_chain,
                    &child_value_options,
                );
            }
        }
    }
}

// Escape string inside single quotes
fn escape_string(string: &str, escape_comma: bool) -> String {
    let string = string.replace('\\', "\\\\").replace('\'', "\\'");
    if escape_comma {
        string.replace(',', "\\,")
    } else {
        string
    }
}

fn escape_help(help: &builder::StyledStr) -> String {
    escape_string(&help.to_string().replace('\n', " "), false)
}

fn escape_name(name: &str) -> String {
    name.replace('-', "_")
}

/// One command in an ordered command chain.
///
/// The leading link is the root command and has `name = None` — its options are stripped but it
/// is not matched as a subcommand token.  Subsequent links have `name = Some(..)` and are matched
/// positionally against the subcommand tokens.
#[derive(Clone)]
struct ChainCommand {
    /// The visible name (or alias) under which this command is reached; `None` for the root.
    name: Option<String>,
    /// Fish `argparse` option specs for every option/flag visible on this command.
    optspecs: Vec<String>,
}

/// Name of the predicate function used for an arbitrarily-nested subcommand chain.
fn chain_helper_name(root_command: &str, chain: &[ChainCommand]) -> String {
    let mut name = format!("__fish_{}_using_command", escape_name(root_command));
    for link in chain.iter().filter(|link| link.name.is_some()) {
        name.push('_');
        name.push_str(&escape_name(link.name.as_deref().unwrap()));
    }
    name
}

/// Build the predicate function for an arbitrarily-nested subcommand and append it to
/// `chain_helpers`.
///
/// The predicate matches the *entire* ordered command chain (rather than a single subcommand
/// token), so two branches that reuse identical subcommand names never share completions.
fn write_chain_helper(
    chain_helpers: &mut String,
    root_command: &str,
    chain: &[ChainCommand],
    value_options: &[String],
    cmd: &Command,
) {
    let name = chain_helper_name(root_command, chain);

    // Merge every level's option specs, letting deeper levels win on collision.  `argparse`
    // rejects a flag defined twice, so global options (present at every level) must collapse to
    // one entry.
    let mut optspecs = std::collections::BTreeMap::new();
    for link in chain {
        for spec in &link.optspecs {
            optspecs.insert(optspec_key(spec), spec.clone());
        }
    }
    let optspecs = optspecs.into_values().collect::<Vec<_>>().join(" ");

    // Number of subcommand links (the nameless root contributes no positional token).
    let depth = chain.iter().filter(|link| link.name.is_some()).count();

    chain_helpers.push_str("function ");
    chain_helpers.push_str(&name);
    chain_helpers.push_str(
        "\n    # Match the full ordered subcommand chain leading here, ignoring options.\n    \
         set -l cmd (commandline -opc)\n    set -e cmd[1]\n    set -q cmd[1]\n    or return 1\n",
    );

    // When the token currently being edited is an option that consumes a value, `commandline`
    // does not yet include that value.  Pad it so `argparse` does not treat the option as a flag.
    if !value_options.is_empty() {
        chain_helpers.push_str("    contains -- $cmd[-1] --");
        for token in value_options {
            chain_helpers.push(' ');
            chain_helpers.push_str(token);
        }
        chain_helpers.push_str("\n    and set -a cmd __fish_clap_complete_value\n");
    }

    chain_helpers.push_str("    argparse -i");
    if !optspecs.is_empty() {
        chain_helpers.push(' ');
        chain_helpers.push_str(&optspecs);
    }
    chain_helpers.push_str(" -- $cmd 2>/dev/null\n    or return 1\n");

    chain_helpers.push_str(&format!(
        "    test (count $argv) -ge {depth}\n    or return 1\n"
    ));
    // Only named (subcommand) links are matched positionally; the root link is skipped.
    for (position, name) in chain
        .iter()
        .filter_map(|link| link.name.as_deref())
        .enumerate()
    {
        let position = position + 1;
        chain_helpers.push_str(&format!(
            "    test \"$argv[{position}]\" = '{}'\n    or return 1\n",
            escape_string(name, false)
        ));
    }

    // Offer this command's own subcommands only until one of them is present, mirroring the
    // shallow `and not __fish_seen_subcommand_from ...` behavior.
    if cmd.has_subcommands() {
        let children = cmd
            .get_subcommands()
            .flat_map(Command::get_name_and_visible_aliases)
            .collect::<Vec<_>>();
        if !children.is_empty() {
            let next = depth + 1;
            for child in children {
                chain_helpers.push_str(&format!(
                    "    contains -- {child} $argv[{next}..]\n    and return 1\n"
                ));
            }
        }
    }

    // A function's status is that of its last command; the final child guard (`contains ... and
    // return 1`) would otherwise fail the whole predicate when no child subcommand is present.
    chain_helpers.push_str("    return 0\nend\n\n");
}

/// Stable key for an `argparse` option spec, used to merge the same option across chain levels.
///
/// Specs look like `s/long`, `s`, or `long`; the long name (if any) or the short char uniquely
/// identifies the flag.  Deeper chain levels overwrite shallower ones.
fn optspec_key(spec: &str) -> String {
    let spec = spec.strip_suffix('=').unwrap_or(spec);
    if let Some((_short, long)) = spec.split_once('/') {
        format!("l:{long}")
    } else if spec.chars().count() == 1 {
        format!("s:{spec}")
    } else {
        format!("l:{spec}")
    }
}

/// All value-taking option tokens (e.g. `--mode`, `-m`) visible on a command, including aliases.
fn command_value_option_tokens(cmd: &Command) -> Vec<String> {
    let mut tokens = Vec::new();
    for arg in cmd.get_arguments().filter(|a| !a.is_positional()) {
        if !arg.get_num_args().map(|r| r.takes_values()).unwrap_or(true) {
            continue;
        }
        if let Some(shorts) = arg.get_short_and_visible_aliases() {
            for short in shorts {
                tokens.push(format!("-{short}"));
            }
        }
        if let Some(longs) = arg.get_long_and_visible_aliases() {
            for long in longs {
                tokens.push(format!("--{}", escape_string(long, false)));
            }
        }
    }
    tokens
}

/// Fish `argparse` option specs for every option and flag visible on a command, including
/// visible aliases.
fn command_optspecs(cmd: &Command) -> Vec<String> {
    let mut specs = Vec::new();
    for arg in cmd.get_arguments().filter(|a| !a.is_positional()) {
        let takes_value = arg.get_num_args().map(|r| r.takes_values()).unwrap_or(true);
        let shorts: Vec<char> = arg.get_short_and_visible_aliases().unwrap_or_default();
        let longs: Vec<&str> = arg.get_long_and_visible_aliases().unwrap_or_default();

        let mut push = |mut spec: String| {
            if takes_value {
                spec.push('=');
            }
            specs.push(spec);
        };

        match (shorts.first(), longs.first()) {
            (Some(short), Some(long)) => push(format!("{short}/{}", escape_string(long, false))),
            (Some(short), None) => push(short.to_string()),
            (None, Some(long)) => push(escape_string(long, false)),
            (None, None) => {}
        }
        for short in shorts.iter().skip(1) {
            push(short.to_string());
        }
        for long in longs.iter().skip(1) {
            push(escape_string(long, false));
        }
    }
    specs
}

/// Print fish's helpers for easy handling subcommands.
fn gen_subcommand_helpers(
    bin_name: &str,
    cmd: &Command,
    buf: &mut dyn Write,
    needs_fn_name: &str,
    using_fn_name: &str,
) {
    let mut optspecs = String::new();
    let cmd_opts = cmd.get_arguments().filter(|a| !a.is_positional());
    for option in cmd_opts {
        optspecs.push(' ');
        let mut has_short = false;
        if let Some(short) = option.get_short() {
            has_short = true;
            optspecs.push(short);
        }

        if let Some(long) = option.get_long() {
            if has_short {
                optspecs.push('/');
            }
            optspecs.push_str(&escape_string(long, false));
        }

        let is_an_option = option
            .get_num_args()
            .map(|r| r.takes_values())
            .unwrap_or(true);
        if is_an_option {
            optspecs.push('=');
        }
    }
    let optspecs_fn_name = format!("__fish_{bin_name}_global_optspecs");
    write!(
        buf,
        "# Print an optspec for argparse to handle cmd's options that are independent of any subcommand.
function {optspecs_fn_name}
    string join \\n{optspecs}
end

function {needs_fn_name}
    # Figure out if the current invocation already has a command.
    set -l cmd (commandline -opc)
    set -e cmd[1]
    argparse -s ({optspecs_fn_name}) -- $cmd 2>/dev/null
    or return
    if set -q argv[1]
        # Also print the command, so this can be used to figure out what it is.
        echo $argv[1]
        return 1
    end
    return 0
end

function {using_fn_name}
    set -l cmd ({needs_fn_name})
    test -z \"$cmd\"
    and return 1
    contains -- $cmd[1] $argv
end

"
        ).expect("failed to write completion file");
}

fn value_completion(option: &Arg) -> String {
    if !option.get_num_args().expect("built").takes_values() {
        return "".to_owned();
    }

    if let Some(data) = utils::possible_values(option) {
        // We return the possible values with their own empty description e.g. "a\t''\nb\t''"
        // this makes sure that a and b don't get the description of the option or argument
        format!(
            " -r -f -a \"{}\"",
            data.iter()
                .filter_map(|value| if value.is_hide_set() {
                    None
                } else {
                    // The help text after \t is wrapped in '' to make sure that the it is taken literally
                    // and there is no command substitution or variable expansion resulting in unexpected errors
                    Some(format!(
                        "{}\\t'{}'",
                        escape_string(value.get_name(), true).as_str(),
                        escape_help(value.get_help().unwrap_or_default())
                    ))
                })
                .collect::<Vec<_>>()
                .join("\n")
        )
    } else {
        // NB! If you change this, please also update the table in `ValueHint` documentation.
        match option.get_value_hint() {
            ValueHint::Unknown => " -r",
            // fish has no built-in support to distinguish these
            ValueHint::AnyPath | ValueHint::FilePath | ValueHint::ExecutablePath => " -r -F",
            ValueHint::DirPath => " -r -f -a \"(__fish_complete_directories)\"",
            // It seems fish has no built-in support for completing command + arguments as
            // single string (CommandString). Complete just the command name.
            ValueHint::CommandString | ValueHint::CommandName => {
                " -r -f -a \"(__fish_complete_command)\""
            }
            ValueHint::Username => " -r -f -a \"(__fish_complete_users)\"",
            ValueHint::Hostname => " -r -f -a \"(__fish_print_hostnames)\"",
            // Disable completion for others
            _ => " -r -f",
        }
        .to_owned()
    }
}
