/// Input, output, and traversal budgets for sanitization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SanitizeLimits {
    /// Maximum UTF-8 input bytes before parsing.
    pub max_input_bytes: usize,
    /// Maximum lexical block/function nesting before recursive parsing.
    pub max_nesting_depth: usize,
    /// Maximum UTF-8 output bytes, checked after serialization.
    pub max_output_bytes: usize,
    /// Maximum nested rule-list depth during AST traversal. Root rules are at 0.
    pub max_rule_depth: usize,
}

impl SanitizeLimits {
    pub fn with_max_input_bytes(mut self, max: usize) -> Self {
        self.max_input_bytes = max;
        self
    }

    /// Sets the lexical nesting budget. Raising it increases upstream parser
    /// stack usage; validate the stack size of the target runtime first.
    pub fn with_max_nesting_depth(mut self, max: usize) -> Self {
        self.max_nesting_depth = max;
        self
    }

    pub fn with_max_output_bytes(mut self, max: usize) -> Self {
        self.max_output_bytes = max;
        self
    }

    pub fn with_max_rule_depth(mut self, max: usize) -> Self {
        self.max_rule_depth = max;
        self
    }
}

impl Default for SanitizeLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 1024 * 1024,
            max_nesting_depth: 32,
            max_output_bytes: 1024 * 1024,
            max_rule_depth: 128,
        }
    }
}

/// Whether parsing recovers from invalid CSS or returns the first error.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseMode {
    /// Recover using lightningcss and include warnings in the sanitization report.
    #[default]
    Recover,
    /// Return a parse error instead of recovering from invalid CSS.
    Strict,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SanitizeOptions {
    pub limits: SanitizeLimits,
    pub parser_flags: lightningcss::stylesheet::ParserFlags,
    pub parse_mode: ParseMode,
    pub(crate) enforce_value_guard: bool,
}

impl SanitizeOptions {
    pub fn with_limits(mut self, limits: SanitizeLimits) -> Self {
        self.limits = limits;
        self
    }

    /// Enables opt-in syntax understood by the configured lightningcss version.
    pub fn with_parser_flags(mut self, flags: lightningcss::stylesheet::ParserFlags) -> Self {
        self.parser_flags = flags;
        self
    }

    pub fn with_parse_mode(mut self, mode: ParseMode) -> Self {
        self.parse_mode = mode;
        self
    }

    /// Disables every engine value/resource invariant. This is intentionally
    /// named as a dangerous capability rather than a routine boolean option.
    pub fn dangerously_disable_value_guard(mut self) -> Self {
        self.enforce_value_guard = false;
        self
    }
}

impl Default for SanitizeOptions {
    fn default() -> Self {
        Self {
            limits: SanitizeLimits::default(),
            parser_flags: lightningcss::stylesheet::ParserFlags::default(),
            parse_mode: ParseMode::Recover,
            enforce_value_guard: true,
        }
    }
}
