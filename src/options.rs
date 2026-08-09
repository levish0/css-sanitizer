//! Configuration for a sanitization pass.

/// Options controlling a sanitization pass.
///
/// Constructed via [`SanitizeOptions::default`] and the `with_*` setters, since
/// the struct is `#[non_exhaustive]` to allow adding fields without a breaking
/// change.
///
/// ```
/// use css_sanitizer::SanitizeOptions;
///
/// let opts = SanitizeOptions::default()
///     .with_max_depth(64)
///     .with_value_guard(true);
/// assert_eq!(opts.max_depth, 64);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct SanitizeOptions {
    /// Maximum nesting depth the sanitizer traverses. Rules nested deeper than
    /// this are dropped (fail-closed) to bound the sanitizer's own recursion.
    ///
    /// Note: this does not bound lightningcss's *parser*, which recurses before
    /// the sanitizer runs. Extremely deeply nested input can overflow the stack
    /// during parsing regardless of this setting; bound untrusted input size
    /// upstream if that is a concern.
    pub max_depth: usize,
    /// Whether to run the engine-enforced value/resource guard over kept
    /// declarations, descriptors, and resource-bearing rules. Enabled by
    /// default.
    ///
    /// Setting this to `false` disables all value-level protection: resources
    /// (`url()`, `src()`, image functions, and `@import`), `var()`, `env()`,
    /// generic functions, and raw tokens are no longer checked, so the policy's
    /// `check_*` hooks become inert. Only disable it if the policy enforces value
    /// safety by other means.
    pub enforce_value_guard: bool,
}

impl SanitizeOptions {
    /// Sets [`max_depth`](Self::max_depth).
    pub fn with_max_depth(mut self, max_depth: usize) -> Self {
        self.max_depth = max_depth;
        self
    }

    /// Sets [`enforce_value_guard`](Self::enforce_value_guard).
    pub fn with_value_guard(mut self, enforce_value_guard: bool) -> Self {
        self.enforce_value_guard = enforce_value_guard;
        self
    }
}

impl Default for SanitizeOptions {
    fn default() -> Self {
        Self {
            max_depth: 256,
            enforce_value_guard: true,
        }
    }
}
