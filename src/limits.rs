//! Lexical input limits, enforced without recursive CSS parsing.

use cssparser::{Parser, ParserInput, Token};

use crate::{SanitizeError, SanitizeLimits};

pub(crate) fn enforce_input_limits(
    input: &str,
    limits: SanitizeLimits,
) -> Result<(), SanitizeError> {
    if input.len() > limits.max_input_bytes {
        return Err(SanitizeError::InputTooLarge {
            actual: input.len(),
            max: limits.max_input_bytes,
        });
    }

    let mut closing_delimiters = Vec::new();
    let mut remaining = input;
    'blocks: loop {
        let mut input = ParserInput::new(remaining);
        let mut parser = Parser::new(&mut input);
        while let Ok(token) = parser.next_including_whitespace_and_comments() {
            let closing_delimiter = match token {
                Token::Function(_) | Token::ParenthesisBlock => b')',
                Token::SquareBracketBlock => b']',
                Token::CurlyBracketBlock => b'}',
                Token::CloseParenthesis | Token::CloseSquareBracket | Token::CloseCurlyBracket => {
                    let closing = match token {
                        Token::CloseParenthesis => b')',
                        Token::CloseSquareBracket => b']',
                        _ => b'}',
                    };
                    if closing_delimiters.last() == Some(&closing) {
                        closing_delimiters.pop();
                    }
                    continue;
                }
                _ => continue,
            };

            if closing_delimiters.len() >= limits.max_nesting_depth {
                return Err(SanitizeError::NestingTooDeep {
                    max: limits.max_nesting_depth,
                });
            }
            closing_delimiters.push(closing_delimiter);

            // Parser normally skips an unopened block on its next call. Start
            // a fresh tokenizer just inside it instead, keeping the matching
            // delimiter stack here and never recursing into parse_nested_block.
            remaining = &remaining[parser.position().byte_index()..];
            continue 'blocks;
        }
        return Ok(());
    }
}

pub(crate) fn validate_output_size(
    output: &str,
    limits: SanitizeLimits,
) -> Result<(), SanitizeError> {
    if output.len() > limits.max_output_bytes {
        return Err(SanitizeError::OutputTooLarge {
            actual: output.len(),
            max: limits.max_output_bytes,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_recursive_shapes_before_parsing() {
        for input in [
            format!("{}a{}{{color:red}}", ":is(".repeat(129), ")".repeat(129)),
            format!("{}a{}{{color:red}}", ":not(".repeat(129), ")".repeat(129)),
            format!("{}color:red{}", "a{".repeat(129), "}".repeat(129)),
            format!(
                "{}a{{color:red}}{}",
                "@media all{".repeat(129),
                "}".repeat(129)
            ),
            format!("width:{}1px{}", "calc(".repeat(129), ")".repeat(129)),
            format!("width:{}1px{}", "min(".repeat(129), ")".repeat(129)),
        ] {
            assert_eq!(
                enforce_input_limits(&input, SanitizeLimits::default()),
                Err(SanitizeError::NestingTooDeep { max: 32 })
            );
        }
    }

    #[test]
    fn follows_css_tokenization_for_strings_comments_escapes_and_urls() {
        let limits = SanitizeLimits::default().with_max_nesting_depth(1);
        for input in [
            r#"content: "(({{"; /* ((( {{{ */ --escaped: \("#,
            "content: \"continued\\\r\n(((\"",
            r"background: url(https://example.test/{[[)",
            r"--escaped: \75\72\6c; --unicode: \é",
        ] {
            assert!(enforce_input_limits(input, limits).is_ok(), "{input}");
        }
    }
}
