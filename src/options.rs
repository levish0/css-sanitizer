/// Parser-facing limits applied before `lightningcss` recursion begins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseLimits {
    pub max_input_bytes: usize,
    pub max_nesting_depth: usize,
    pub max_output_bytes: usize,
}

impl ParseLimits {
    pub fn with_max_input_bytes(mut self, max: usize) -> Self {
        self.max_input_bytes = max;
        self
    }

    pub fn with_max_nesting_depth(mut self, max: usize) -> Self {
        self.max_nesting_depth = max;
        self
    }

    pub fn with_max_output_bytes(mut self, max: usize) -> Self {
        self.max_output_bytes = max;
        self
    }
}

impl Default for ParseLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 1024 * 1024,
            max_nesting_depth: 128,
            max_output_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SanitizeOptions {
    pub parse_limits: ParseLimits,
    pub max_traversal_depth: usize,
    pub parser_flags: lightningcss::stylesheet::ParserFlags,
    pub error_recovery: bool,
    pub(crate) enforce_value_guard: bool,
}

impl SanitizeOptions {
    pub fn with_parse_limits(mut self, parse_limits: ParseLimits) -> Self {
        self.parse_limits = parse_limits;
        self
    }

    pub fn with_max_traversal_depth(mut self, max_depth: usize) -> Self {
        self.max_traversal_depth = max_depth;
        self
    }

    /// Enables opt-in syntax understood by the configured lightningcss version.
    pub fn with_parser_flags(mut self, flags: lightningcss::stylesheet::ParserFlags) -> Self {
        self.parser_flags = flags;
        self
    }

    /// Rejects the first invalid rule or declaration instead of recovering.
    pub fn with_strict_parsing(mut self) -> Self {
        self.error_recovery = false;
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
            parse_limits: ParseLimits::default(),
            max_traversal_depth: 128,
            parser_flags: lightningcss::stylesheet::ParserFlags::default(),
            error_recovery: true,
            enforce_value_guard: true,
        }
    }
}
