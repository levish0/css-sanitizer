use std::fmt;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SanitizeReport {
    pub dropped_rules: usize,
    pub dropped_selector_lists: usize,
    pub dropped_declarations: usize,
    pub dropped_descriptors: usize,
    pub rejected_values: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedCss(String);

impl SanitizedCss {
    pub(crate) fn new(css: String) -> Self {
        Self(css)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn into_string(self) -> String {
        self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Serializes CSS for the HTML raw-text contents of a `<style>` element.
    pub fn to_style_element_text(&self) -> String {
        escape_style_close_tag(&self.0)
    }
}

impl AsRef<str> for SanitizedCss {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SanitizedCss {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizeOutput {
    pub css: SanitizedCss,
    pub report: SanitizeReport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SanitizeError {
    InputTooLarge { actual: usize, max: usize },
    NestingTooDeep { max: usize },
    Parse(String),
    Serialize(String),
    OutputTooLarge { actual: usize, max: usize },
}

impl fmt::Display for SanitizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputTooLarge { actual, max } => {
                write!(f, "CSS input is {actual} bytes; limit is {max}")
            }
            Self::NestingTooDeep { max } => {
                write!(f, "CSS nesting exceeds the pre-parse limit of {max}")
            }
            Self::Parse(error) => write!(f, "CSS parse failed: {error}"),
            Self::Serialize(error) => write!(f, "CSS serialization failed: {error}"),
            Self::OutputTooLarge { actual, max } => {
                write!(f, "sanitized CSS output is {actual} bytes; limit is {max}")
            }
        }
    }
}

impl std::error::Error for SanitizeError {}

fn escape_style_close_tag(value: &str) -> String {
    fn is_boundary(byte: u8) -> bool {
        byte.is_ascii_whitespace() || matches!(byte, b'>' | b'/')
    }

    let bytes = value.as_bytes();
    let mut output = String::with_capacity(value.len());
    let mut cursor = 0;
    while let Some(relative) = value[cursor..].find("</") {
        let start = cursor + relative;
        output.push_str(&value[cursor..start]);
        let name_start = start + 2;
        let name_end = name_start + 5;
        if name_end <= bytes.len()
            && bytes[name_start..name_end].eq_ignore_ascii_case(b"style")
            && (name_end == bytes.len() || is_boundary(bytes[name_end]))
        {
            output.push_str("<\\/");
            output.push_str(&value[name_start..name_end]);
            cursor = name_end;
        } else {
            output.push_str("</");
            cursor = name_start;
        }
    }
    output.push_str(&value[cursor..]);
    output
}
