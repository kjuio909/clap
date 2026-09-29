use std::ffi::OsStr;
use std::ffi::OsString;

use clap::builder::StyledStr;

/// A shell-agnostic completion candidate
#[derive(Default, Debug)]
pub struct CompletionCandidate {
    value: OsString,
    /// Literal text of the token prefix the value is attached to
    /// (`--option=` or `-f`), kept separate from [`value`] so that the
    /// candidate text matches the independent (`--option value`) spelling.
    /// Shell adapters re-attach it when emitting the replacement text.
    token_prefix: OsString,
    help: Option<StyledStr>,
    id: Option<String>,
    tag: Option<StyledStr>,
    display_order: Option<usize>,
    hidden: bool,
}

impl PartialEq for CompletionCandidate {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
            && self.help == other.help
            && self.id == other.id
            && self.tag == other.tag
            && self.display_order == other.display_order
            && self.hidden == other.hidden
    }
}

impl Eq for CompletionCandidate {}

impl PartialOrd for CompletionCandidate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CompletionCandidate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.value
            .cmp(&other.value)
            .then_with(|| self.help.cmp(&other.help))
            .then_with(|| self.id.cmp(&other.id))
            .then_with(|| self.tag.cmp(&other.tag))
            .then_with(|| self.display_order.cmp(&other.display_order))
            .then_with(|| self.hidden.cmp(&other.hidden))
    }
}

impl CompletionCandidate {
    /// Create a new completion candidate
    pub fn new(value: impl Into<OsString>) -> Self {
        let value = value.into();
        Self {
            value,
            ..Default::default()
        }
    }

    /// Set the help message of the completion candidate
    pub fn help(mut self, help: Option<StyledStr>) -> Self {
        self.help = help;
        self
    }

    /// Only first for a given Id is shown
    ///
    /// To reduce the risk of conflicts, this should likely contain a namespace.
    pub fn id(mut self, id: Option<String>) -> Self {
        self.id = id;
        self
    }

    /// Group candidates by tag
    ///
    /// Future: these may become user-visible
    pub fn tag(mut self, tag: Option<StyledStr>) -> Self {
        self.tag = tag;
        self
    }

    /// Sort weight within a [`CompletionCandidate::tag`]
    pub fn display_order(mut self, order: Option<usize>) -> Self {
        self.display_order = order;
        self
    }

    /// Set the visibility of the completion candidate
    ///
    /// Only shown when there is no visible candidate for completing the current argument.
    pub fn hide(mut self, hidden: bool) -> Self {
        self.hidden = hidden;
        self
    }

    /// Add a prefix to the value of completion candidate
    ///
    /// This is generally used for post-process by [`complete`][crate::engine::complete()] for
    /// things like prepending flags, merging delimiter-separated values, etc.
    pub fn add_prefix(mut self, prefix: impl Into<OsString>) -> Self {
        let suffix = self.value;
        let mut value = prefix.into();
        value.push(&suffix);
        self.value = value;
        self
    }

    /// Record the already-typed token text (`--option=` or `-f`) that the
    /// candidate value completes within.
    ///
    /// Unlike [`add_prefix`], this leaves [`get_value`](Self::get_value) bare,
    /// so attached (`--option=val`) and independent (`--option val`)
    /// completions carry identical candidate text. Shell adapters that replace
    /// the whole token re-attach this via
    /// [`get_token_prefix`](Self::get_token_prefix).
    pub(crate) fn token_prefix(mut self, prefix: impl Into<OsString>) -> Self {
        self.token_prefix = prefix.into();
        self
    }
}

/// Reflection API
impl CompletionCandidate {
    /// Get the literal value being proposed for completion
    pub fn get_value(&self) -> &OsStr {
        &self.value
    }

    /// Get the already-typed token text (`--option=` or `-f`) the value is
    /// attached to, if completing an attached option value.
    ///
    /// [`get_value`](Self::get_value) is always bare. Shell adapters that
    /// replace the token under the cursor should prepend this prefix when
    /// emitting the completion.
    pub(crate) fn get_token_prefix(&self) -> &OsStr {
        &self.token_prefix
    }

    /// Get the help message of the completion candidate
    pub fn get_help(&self) -> Option<&StyledStr> {
        self.help.as_ref()
    }

    /// Get the id used for de-duplicating
    pub fn get_id(&self) -> Option<&String> {
        self.id.as_ref()
    }

    /// Get the grouping tag
    pub fn get_tag(&self) -> Option<&StyledStr> {
        self.tag.as_ref()
    }

    /// Get the grouping tag
    pub fn get_display_order(&self) -> Option<usize> {
        self.display_order
    }

    /// Get the visibility of the completion candidate
    pub fn is_hide_set(&self) -> bool {
        self.hidden
    }
}

impl<S: Into<OsString>> From<S> for CompletionCandidate {
    fn from(s: S) -> Self {
        Self::new(s.into())
    }
}
