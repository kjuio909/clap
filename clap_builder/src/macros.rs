/// Allows you to pull the version from your Cargo.toml at compile time as
/// `MAJOR.MINOR.PATCH_PKGVERSION_PRE`
///
/// # Examples
///
/// ```no_run
/// # use clap_builder as clap;
/// # use clap::crate_version;
/// # use clap::Command;
/// let m = Command::new("cmd")
///             .version(crate_version!())
///             .get_matches();
/// ```
#[cfg(feature = "cargo")]
#[macro_export]
macro_rules! crate_version {
    () => {
        env!("CARGO_PKG_VERSION")
    };
}

/// Allows you to pull the authors for the command from your Cargo.toml at
/// compile time in the form:
/// `"author1 lastname <author1@example.com>:author2 lastname <author2@example.com>"`
///
/// You can replace the colons with a custom separator by supplying a
/// replacement string, so, for example,
/// `crate_authors!(",\n")` would become
/// `"author1 lastname <author1@example.com>,\nauthor2 lastname <author2@example.com>,\nauthor3 lastname <author3@example.com>"`
///
/// # Examples
///
/// ```no_run
/// # use clap_builder as clap;
/// # use clap::crate_authors;
/// # use clap::Command;
/// let m = Command::new("cmd")
///             .author(crate_authors!("\n"))
///             .get_matches();
/// ```
#[cfg(feature = "cargo")]
#[macro_export]
macro_rules! crate_authors {
    ($sep:expr) => {{
        static AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
        if AUTHORS.contains(':') {
            static CACHED: std::sync::OnceLock<String> = std::sync::OnceLock::new();
            let s = CACHED.get_or_init(|| AUTHORS.replace(':', $sep));
            let s: &'static str = &*s;
            s
        } else {
            AUTHORS
        }
    }};
    () => {
        env!("CARGO_PKG_AUTHORS")
    };
}

/// Allows you to pull the description from your Cargo.toml at compile time.
///
/// # Examples
///
/// ```no_run
/// # use clap_builder as clap;
/// # use clap::crate_description;
/// # use clap::Command;
/// let m = Command::new("cmd")
///             .about(crate_description!())
///             .get_matches();
/// ```
#[cfg(feature = "cargo")]
#[macro_export]
macro_rules! crate_description {
    () => {
        env!("CARGO_PKG_DESCRIPTION")
    };
}

/// Allows you to pull the name from your Cargo.toml at compile time.
///
/// <div class="warning">
///
/// **NOTE:** This macro extracts the name from an environment variable `CARGO_PKG_NAME`.
/// When the crate name is set to something different from the package name,
/// use environment variables `CARGO_CRATE_NAME` or `CARGO_BIN_NAME`.
/// See [the Cargo Book](https://doc.rust-lang.org/cargo/reference/environment-variables.html)
/// for more information.
///
/// </div>
///
/// # Examples
///
/// ```no_run
/// # use clap_builder as clap;
/// # use clap::crate_name;
/// # use clap::Command;
/// let m = Command::new(crate_name!())
///             .get_matches();
/// ```
#[cfg(feature = "cargo")]
#[macro_export]
macro_rules! crate_name {
    () => {
        env!("CARGO_PKG_NAME")
    };
}

/// Allows you to build the `Command` instance from your Cargo.toml at compile time.
///
/// <div class="warning">
///
/// **NOTE:** Changing the values in your `Cargo.toml` does not trigger a re-build automatically,
/// and therefore won't change the generated output until you recompile.
///
/// In some cases you can "trick" the compiler into triggering a rebuild when your
/// `Cargo.toml` is changed by including this in your `src/main.rs` file
/// `include_str!("../Cargo.toml");`
///
/// </div>
///
/// # Examples
///
/// ```no_run
/// # use clap_builder as clap;
/// # use clap::command;
/// let m = command!().get_matches();
/// ```
#[cfg(feature = "cargo")]
#[macro_export]
macro_rules! command {
    () => {{ $crate::command!($crate::crate_name!()) }};
    ($name:expr) => {{
        let mut cmd = $crate::Command::new($name).version($crate::crate_version!());

        let author = $crate::crate_authors!();
        if !author.is_empty() {
            cmd = cmd.author(author)
        }

        let about = $crate::crate_description!();
        if !about.is_empty() {
            cmd = cmd.about(about)
        }

        cmd
    }};
}

/// Requires `cargo` feature flag to be enabled.
#[cfg(not(feature = "cargo"))]
#[macro_export]
macro_rules! command {
    () => {{
        compile_error!("`cargo` feature flag is required");
    }};
    ($name:expr) => {{
        compile_error!("`cargo` feature flag is required");
    }};
}

#[doc(hidden)]
#[macro_export]
macro_rules! arg_impl {
    ( @string $val:ident ) => {
        stringify!($val)
    };
    ( @string $val:literal ) => {{
        let ident_or_string_literal: &str = $val;
        ident_or_string_literal
    }};
    ( @string $val:tt ) => {
        ::std::compile_error!("Only identifiers or string literals supported");
    };
    ( @string ) => {
        None
    };

    ( @char $val:ident ) => {{
        let ident_or_char_literal = stringify!($val);
        debug_assert_eq!(
            ident_or_char_literal.len(),
            1,
            "Single-letter identifier expected, got {ident_or_char_literal}",
        );
        ident_or_char_literal.chars().next().unwrap()
    }};
    ( @char $val:literal ) => {{
        let ident_or_char_literal: char = $val;
        ident_or_char_literal
    }};
    ( @char ) => {{
        None
    }};

    (
        @arg
        ($arg:expr)
        --$long:ident
        $($tail:tt)*
    ) => {{
        debug_assert_eq!($arg.get_value_names(), None, "Flags should precede values");
        debug_assert!(!matches!($arg.get_action(), $crate::ArgAction::Append), "Flags should precede `...`");

        let mut arg = $arg;
        let long = $crate::arg_impl! { @string $long };
        if arg.get_id() == "" {
            arg = arg.id(long);
        }
        let action = $crate::ArgAction::SetTrue;
        let arg = arg
            .long(long)
            .action(action);
        let arg = $crate::arg_impl! {
            @arg (arg) $($tail)*
        };
        arg
    }};
    (
        @arg
        ($arg:expr)
        --$long:literal
        $($tail:tt)*
    ) => {{
        debug_assert_eq!($arg.get_value_names(), None, "Flags should precede values");
        debug_assert!(!matches!($arg.get_action(), $crate::ArgAction::Append), "Flags should precede `...`");

        let mut arg = $arg;
        let long = $crate::arg_impl! { @string $long };
        if arg.get_id() == "" {
            arg = arg.id(long);
        }
        let action = $crate::ArgAction::SetTrue;
        let arg = arg
            .long(long)
            .action(action);
        let arg = $crate::arg_impl! {
            @arg (arg) $($tail)*
        };
        arg
    }};
    (
        @arg
        ($arg:expr)
        -$short:ident
        $($tail:tt)*
    ) => {{
        debug_assert_eq!($arg.get_long(), None, "Short flags should precede long flags");
        debug_assert_eq!($arg.get_value_names(), None, "Flags should precede values");
        debug_assert!(!matches!($arg.get_action(), $crate::ArgAction::Append), "Flags should precede `...`");

        let action = $crate::ArgAction::SetTrue;
        let arg = $arg
            .short($crate::arg_impl! { @char $short })
            .action(action);
        let arg = $crate::arg_impl! {
            @arg (arg) $($tail)*
        };
        arg
    }};
    (
        @arg
        ($arg:expr)
        -$short:literal
        $($tail:tt)*
    ) => {{
        debug_assert_eq!($arg.get_long(), None, "Short flags should precede long flags");
        debug_assert_eq!($arg.get_value_names(), None, "Flags should precede values");
        debug_assert!(!matches!($arg.get_action(), $crate::ArgAction::Append), "Flags should precede `...`");

        let action = $crate::ArgAction::SetTrue;
        let arg = $arg
            .short($crate::arg_impl! { @char $short })
            .action(action);
        let arg = $crate::arg_impl! {
            @arg (arg) $($tail)*
        };
        arg
    }};
    (
        @arg
        ($arg:expr)
        <$value_name:ident>
        $($tail:tt)*
    ) => {{
        debug_assert!(!matches!($arg.get_action(), $crate::ArgAction::Append), "Flags should precede `...`");

        let mut arg = $arg;

        if arg.get_long().is_none() && arg.get_short().is_none() {
            arg = arg.required(true);
        }

        let value_name = $crate::arg_impl! { @string $value_name };
        if arg.get_id() == "" {
            arg = arg.id(value_name);
        }
        let arg = arg
            .value_name(value_name)
            .action($crate::ArgAction::Set);
        $crate::arg_impl! {
            @next (arg) (1, 1, 0) [$crate::arg_impl!(@string $value_name),] $($tail)*
        }
    }};
    (
        @arg
        ($arg:expr)
        <$value_name:literal>
        $($tail:tt)*
    ) => {{
        debug_assert!(!matches!($arg.get_action(), $crate::ArgAction::Append), "Flags should precede `...`");

        let mut arg = $arg;

        if arg.get_long().is_none() && arg.get_short().is_none() {
            arg = arg.required(true);
        }

        let value_name = $crate::arg_impl! { @string $value_name };
        if arg.get_id() == "" {
            arg = arg.id(value_name);
        }
        let arg = arg
            .value_name(value_name)
            .action($crate::ArgAction::Set);
        $crate::arg_impl! {
            @next (arg) (1, 1, 0) [$crate::arg_impl!(@string $value_name),] $($tail)*
        }
    }};
    (
        @arg
        ($arg:expr)
        [$value_name:ident]
        $($tail:tt)*
    ) => {{
        debug_assert!(!matches!($arg.get_action(), $crate::ArgAction::Append), "Flags should precede `...`");

        let mut arg = $arg;

        if arg.get_long().is_none() && arg.get_short().is_none() {
            arg = arg.required(false);
        } else {
            arg = arg.num_args(0..=1);
        }

        let value_name = $crate::arg_impl! { @string $value_name };
        if arg.get_id() == "" {
            arg = arg.id(value_name);
        }
        let arg = arg
            .value_name(value_name)
            .action($crate::ArgAction::Set);
        $crate::arg_impl! {
            @next (arg) (0, 1, 1) [$crate::arg_impl!(@string $value_name),] $($tail)*
        }
    }};
    (
        @arg
        ($arg:expr)
        [$value_name:literal]
        $($tail:tt)*
    ) => {{
        debug_assert!(!matches!($arg.get_action(), $crate::ArgAction::Append), "Flags should precede `...`");

        let mut arg = $arg;

        if arg.get_long().is_none() && arg.get_short().is_none() {
            arg = arg.required(false);
        } else {
            arg = arg.num_args(0..=1);
        }

        let value_name = $crate::arg_impl! { @string $value_name };
        if arg.get_id() == "" {
            arg = arg.id(value_name);
        }
        let arg = arg
            .value_name(value_name)
            .action($crate::ArgAction::Set);
        $crate::arg_impl! {
            @next (arg) (0, 1, 1) [$crate::arg_impl!(@string $value_name),] $($tail)*
        }
    }};
    // Continuation of a value-placeholder sequence.
    // State is `($min, $max, $optional)` plus `[$names:expr, ...]`, the placeholder names
    // collected so far.  `$optional` records whether an optional (`[NAME]`) was seen.
    (
        @next
        ($arg:expr)
        ($min:expr, $max:expr, $optional:expr)
        [$($names:expr),* $(,)?]
        <$value_name:ident>
        $($tail:tt)*
    ) => {{
        debug_assert_eq!(
            $optional, 0,
            "Required value placeholders must precede optional value placeholders"
        );

        $crate::arg_impl! {
            @next ($arg) ($min + 1, $max + 1, $optional)
            [$($names,)* $crate::arg_impl!(@string $value_name)]
            $($tail)*
        }
    }};
    (
        @next
        ($arg:expr)
        ($min:expr, $max:expr, $optional:expr)
        [$($names:expr),* $(,)?]
        <$value_name:literal>
        $($tail:tt)*
    ) => {{
        debug_assert_eq!(
            $optional, 0,
            "Required value placeholders must precede optional value placeholders"
        );

        $crate::arg_impl! {
            @next ($arg) ($min + 1, $max + 1, $optional)
            [$($names,)* $crate::arg_impl!(@string $value_name)]
            $($tail)*
        }
    }};
    (
        @next
        ($arg:expr)
        ($min:expr, $max:expr, $optional:expr)
        [$($names:expr),* $(,)?]
        [$value_name:ident]
        $($tail:tt)*
    ) => {{
        $crate::arg_impl! {
            @next ($arg) ($min, $max + 1, 1)
            [$($names,)* $crate::arg_impl!(@string $value_name)]
            $($tail)*
        }
    }};
    (
        @next
        ($arg:expr)
        ($min:expr, $max:expr, $optional:expr)
        [$($names:expr),* $(,)?]
        [$value_name:literal]
        $($tail:tt)*
    ) => {{
        $crate::arg_impl! {
            @next ($arg) ($min, $max + 1, 1)
            [$($names,)* $crate::arg_impl!(@string $value_name)]
            $($tail)*
        }
    }};
    // Placeholders followed by `...`.  A single placeholder keeps its historical meaning
    // (repeated occurrences); multiple placeholders keep accepting values past the fixed ones.
    (
        @next
        ($arg:expr)
        ($min:expr, $max:expr, $optional:expr)
        [$($names:expr),* $(,)?]
        ...
        $($tail:tt)*
    ) => {{
        let _ = $optional;
        if $max > 1 {
            let arg = $arg
                .value_names([$($names),*])
                .num_args($min..)
                // Allow collecting arguments interleaved with flags
                .action($crate::ArgAction::Append);
            $crate::arg_impl! {
                @arg (arg) $($tail)*
            }
        } else {
            $crate::arg_impl! {
                @arg ($arg) ... $($tail)*
            }
        }
    }};
    // Placeholders followed by a help string (or nothing): finalize the placeholder names
    // and `num_args`.  A lone placeholder keeps the settings chosen by its first arm
    // (unset value count for a required value, `0..=1` for an optional named value).
    (
        @next
        ($arg:expr)
        ($min:expr, $max:expr, $optional:expr)
        [$($names:expr),* $(,)?]
        $help:literal
    ) => {{
        let _ = $optional;
        let arg = if $max > 1 {
            $arg.value_names([$($names),*]).num_args($min..=$max)
        } else {
            $arg
        };
        arg.help($help)
    }};
    (
        @next
        ($arg:expr)
        ($min:expr, $max:expr, $optional:expr)
        [$($names:expr),* $(,)?]
    ) => {{
        let _ = $optional;
        if $max > 1 {
            $arg.value_names([$($names),*]).num_args($min..=$max)
        } else {
            $arg
        }
    }};
    // Anything else after a value placeholder is unsupported (e.g. a flag out of order).
    (
        @next
        ($arg:expr)
        ($min:expr, $max:expr, $optional:expr)
        [$($names:expr),* $(,)?]
        $($tail:tt)+
    ) => {{
        let _ = ($min, $max, $optional, $($names),*);
        compile_error!(
            "Expected value placeholders (`<NAME>`/`[NAME]`), `...`, or a help string; \
             flags must precede value placeholders"
        )
    }};
    (
        @arg
        ($arg:expr)
        ...
        $($tail:tt)*
    ) => {{
        let arg = match $arg.get_action() {
            $crate::ArgAction::Set => {
                if $arg.get_long().is_none() && $arg.get_short().is_none() {
                    $arg.num_args(1..)
                        // Allow collecting arguments interleaved with flags
                        .action($crate::ArgAction::Append)
                } else {
                    $arg.action($crate::ArgAction::Append)
                }
            },
            $crate::ArgAction::SetTrue | $crate::ArgAction::Help | $crate::ArgAction::Version => {
                $arg.action($crate::ArgAction::Count)
            }
            action => {
                panic!("Unexpected action {action:?}")
            }
        };
        let arg = $crate::arg_impl! {
            @arg (arg) $($tail)*
        };
        arg
    }};
    (
        @arg
        ($arg:expr)
        $help:literal
    ) => {{
        $arg.help($help)
    }};
    (
        @arg
        ($arg:expr)
    ) => {{
        $arg
    }};
}

/// Create an [`Arg`] from a usage string.
///
/// Allows creation of basic settings for the [`Arg`].
///
/// <div class="warning">
///
/// **NOTE**: Not all settings may be set using the usage string method. Some properties are
/// only available via the builder pattern.
///
/// </div>
///
/// # Syntax
///
/// Usage strings typically following the form:
///
/// ```notrust
/// [explicit name] [short] [long] [value names] [...] [help string]
/// ```
///
/// ### Explicit Name
///
/// The name may be either a bare-word or a string, followed by a `:`, like `name:` or
/// `"name":`.
///
/// *Note:* This is an optional field, if it's omitted the argument will use one of the additional
/// fields as the name using the following priority order:
///
///  1. Explicit Name
///  2. Long
///  3. Value Name
///
/// See [`Arg::id`][crate::Arg::id].
///
/// ### Short
///
/// A short flag is a `-` followed by either a bare-character or quoted character, like `-f` or
/// `-'f'`.
///
/// See [`Arg::short`][crate::Arg::short].
///
/// ### Long
///
/// A long flag is a `--` followed by either a bare-word or a string, like `--foo` or
/// `--"foo"`.
///
/// <div class="warning">
///
/// **NOTE:** Dashes in the long name (e.g. `--foo-bar`) is not supported and quoting is required
/// (e.g. `--"foo-bar"`).
///
/// </div>
///
/// See [`Arg::long`][crate::Arg::long].
///
/// ### Values (Value Notation)
///
/// This is set by placing bare-word between:
/// - `[]` like `[FOO]`
///   - Positional argument: optional
///   - Named argument: optional value
/// - `<>` like `<FOO>`: required
///
/// Multiple placeholders may follow one another to declare several values per occurrence,
/// e.g. `--copy <SRC> <DST> [MODE]`.  The placeholder order is preserved in
/// [`Arg::get_value_names`][crate::Arg::get_value_names] and the accepted value count is derived
/// from them (here, two required and one optional value, i.e. `2..=3`).  Required placeholders
/// (`<FOO>`) must precede optional ones (`[FOO]`).
///
/// See [`Arg::value_name`][crate::Arg::value_name] and
/// [`Arg::value_names`][crate::Arg::value_names].
///
/// ### `...`
///
/// `...` (three consecutive dots/periods) specifies that this argument may occur multiple
/// times (not to be confused with multiple values per occurrence).
///
/// See [`ArgAction::Count`][crate::ArgAction::Count] and [`ArgAction::Append`][crate::ArgAction::Append].
///
/// ### Help String
///
/// The help string is denoted between a pair of double quotes `""` and may contain any
/// characters.
///
/// # Examples
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Command, Arg, arg};
/// let cmd = Command::new("prog")
///     .args(&[
///         arg!(--config <FILE> "a required file for the configuration and no short"),
///         arg!(-d --debug ... "turns on debugging information and allows multiples"),
///         arg!([input] "an optional input file to use")
///     ]);
///
/// let m = cmd.try_get_matches_from(["prog", "--config", "file.toml"]).unwrap();
/// assert_eq!(m.get_one::<String>("config").unwrap(), "file.toml");
/// assert_eq!(*m.get_one::<u8>("debug").unwrap(), 0);
/// assert_eq!(m.get_one::<String>("input"), None);
/// ```
///
/// An argument can take several values per occurrence by listing several placeholders:
///
/// ```rust
/// # use clap_builder as clap;
/// # use clap::{Command, arg};
/// let cmd = Command::new("prog").arg(arg!(--copy <SRC> <DST> [MODE]));
///
/// // Two values (the optional `MODE` is omitted)
/// let m = cmd
///     .clone()
///     .try_get_matches_from(["prog", "--copy", "a", "b"])
///     .unwrap();
/// assert_eq!(
///     m.get_many::<String>("copy").unwrap().collect::<Vec<_>>(),
///     vec!["a", "b"]
/// );
///
/// // All three values, in declaration order
/// let m = cmd
///     .try_get_matches_from(["prog", "--copy", "a", "b", "fast"])
///     .unwrap();
/// assert_eq!(
///     m.get_many::<String>("copy").unwrap().collect::<Vec<_>>(),
///     vec!["a", "b", "fast"]
/// );
/// ```
/// [`Arg`]: crate::Arg
#[macro_export]
macro_rules! arg {
    ( -$($tail:tt)+ ) => {{
        let arg = $crate::Arg::default();
        let arg = $crate::arg_impl! {
            @arg (arg) -$($tail)+
        };
        debug_assert_ne!(arg.get_id(), "", "Without a value or long flag, the `name:` prefix is required");
        arg
    }};
    ( $name:ident: $($tail:tt)+ ) => {{
        let arg = $crate::Arg::new($crate::arg_impl! { @string $name });
        let arg = $crate::arg_impl! {
            @arg (arg) $($tail)+
        };
        arg
    }};
    ( $name:literal: $($tail:tt)+ ) => {{
        let arg = $crate::Arg::new($crate::arg_impl! { @string $name });
        let arg = $crate::arg_impl! {
            @arg (arg) $($tail)+
        };
        arg
    }};
    ( $($tail:tt)+ ) => {{
        let arg = $crate::Arg::default();
        let arg = $crate::arg_impl! {
            @arg (arg) $($tail)+
        };
        debug_assert_ne!(arg.get_id(), "", "Without a value or long flag, the `name:` prefix is required");
        arg
    }};
}

#[cfg(feature = "debug")]
macro_rules! debug {
    ($($arg:tt)*) => ({
        use std::fmt::Write as _;
        let hint = anstyle::Style::new().dimmed();

        let module_path = module_path!();
        let body = format!($($arg)*);
        let mut styled = $crate::builder::StyledStr::new();
        let _ = write!(styled, "{hint}[{module_path:>28}]{body}{hint:#}\n");
        let color = $crate::output::fmt::Colorizer::new($crate::output::fmt::Stream::Stderr, $crate::ColorChoice::Auto).with_content(styled);
        let _ = color.print();
    })
}

#[cfg(not(feature = "debug"))]
macro_rules! debug {
    ($($arg:tt)*) => {};
}

macro_rules! ok {
    ($expr:expr) => {
        match $expr {
            Ok(val) => val,
            Err(err) => {
                return Err(err);
            }
        }
    };
}

macro_rules! some {
    ($expr:expr) => {
        match $expr {
            Some(val) => val,
            None => {
                return None;
            }
        }
    };
}
