//! Public entry points: enforce budgets, parse, sanitize the AST, and serialize.

use std::sync::{Arc, RwLock};

use crate::ast_sanitizer::AstSanitizer;
use crate::limits::{enforce_input_limits, validate_output_size};
use crate::options::{ParseMode, SanitizeOptions};
use crate::output::{SanitizeError, SanitizeOutput, SanitizeReport, SanitizedCss};
use crate::policy::CssPolicy;
use lightningcss::declaration::DeclarationBlock;
use lightningcss::printer::{Printer, PrinterOptions};
use lightningcss::stylesheet::{ParserOptions, StyleSheet};
use lightningcss::traits::ToCss;

fn serialize_declaration_block(block: &DeclarationBlock<'_>) -> Result<String, SanitizeError> {
    let mut output = String::new();
    let mut printer = Printer::new(&mut output, PrinterOptions::default());
    block
        .to_css(&mut printer)
        .map_err(|error| SanitizeError::Serialize(error.to_string()))?;
    Ok(output)
}

/// Sanitizes an existing declaration block in place using default options.
/// The caller owns the original parse and output budgets and must provide a
/// structurally valid lightningcss AST. Raw token values are revalidated.
pub fn sanitize_declaration_block_ast(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssPolicy,
) -> SanitizeReport {
    sanitize_declaration_block_ast_with_options(block, policy, SanitizeOptions::default())
}

/// Sanitizes an existing declaration block with explicit options.
/// See [`sanitize_declaration_block_ast`] for the AST trust boundary.
pub fn sanitize_declaration_block_ast_with_options(
    block: &mut DeclarationBlock<'_>,
    policy: &dyn CssPolicy,
    options: SanitizeOptions,
) -> SanitizeReport {
    AstSanitizer::new(policy, options).sanitize_declaration_block(block)
}

/// Sanitizes an existing stylesheet in place using default options.
/// The caller owns the original parse and output budgets and must provide a
/// structurally valid lightningcss AST. Raw token values are revalidated.
pub fn sanitize_stylesheet_ast(
    stylesheet: &mut StyleSheet<'_>,
    policy: &dyn CssPolicy,
) -> SanitizeReport {
    sanitize_stylesheet_ast_with_options(stylesheet, policy, SanitizeOptions::default())
}

/// Sanitizes an existing stylesheet with explicit options.
/// See [`sanitize_stylesheet_ast`] for the AST trust boundary.
pub fn sanitize_stylesheet_ast_with_options(
    stylesheet: &mut StyleSheet<'_>,
    policy: &dyn CssPolicy,
    options: SanitizeOptions,
) -> SanitizeReport {
    AstSanitizer::new(policy, options).sanitize_stylesheet(stylesheet)
}

/// Parses and sanitizes declarations such as an inline style using default options.
/// Returns raw CSS and a report of policy removals and parser recovery warnings.
pub fn sanitize_declaration_list(
    input: &str,
    policy: &dyn CssPolicy,
) -> Result<SanitizeOutput, SanitizeError> {
    sanitize_declaration_list_with_options(input, policy, SanitizeOptions::default())
}

/// Parses and sanitizes declarations with explicit parser and budget options.
pub fn sanitize_declaration_list_with_options(
    input: &str,
    policy: &dyn CssPolicy,
    options: SanitizeOptions,
) -> Result<SanitizeOutput, SanitizeError> {
    enforce_input_limits(input, options.limits)?;
    let limits = options.limits;
    let warnings = Arc::new(RwLock::new(Vec::new()));
    let parser_options = ParserOptions {
        error_recovery: matches!(options.parse_mode, ParseMode::Recover),
        flags: options.parser_flags.clone(),
        warnings: Some(Arc::clone(&warnings)),
        ..ParserOptions::default()
    };
    let mut block = DeclarationBlock::parse_string(input, parser_options)
        .map_err(|error| SanitizeError::Parse(error.to_string()))?;
    let mut report = sanitize_declaration_block_ast_with_options(&mut block, policy, options);
    report.record_parse_warnings(&warnings.read().unwrap_or_else(|error| error.into_inner()));
    let css = if block.is_empty() {
        String::new()
    } else {
        serialize_declaration_block(&block)?
    };
    validate_output_size(&css, limits)?;
    Ok(SanitizeOutput {
        css: SanitizedCss::new(css),
        report,
    })
}

/// Parses and sanitizes a complete stylesheet using default options.
/// Returns raw CSS and a report of policy removals and parser recovery warnings.
pub fn sanitize_stylesheet(
    input: &str,
    policy: &dyn CssPolicy,
) -> Result<SanitizeOutput, SanitizeError> {
    sanitize_stylesheet_with_options(input, policy, SanitizeOptions::default())
}

/// Parses and sanitizes a complete stylesheet with explicit parser and budget options.
pub fn sanitize_stylesheet_with_options(
    input: &str,
    policy: &dyn CssPolicy,
    options: SanitizeOptions,
) -> Result<SanitizeOutput, SanitizeError> {
    enforce_input_limits(input, options.limits)?;
    let limits = options.limits;
    let warnings = Arc::new(RwLock::new(Vec::new()));
    let parser_options = ParserOptions {
        error_recovery: matches!(options.parse_mode, ParseMode::Recover),
        flags: options.parser_flags.clone(),
        warnings: Some(Arc::clone(&warnings)),
        ..ParserOptions::default()
    };
    let mut stylesheet = StyleSheet::parse(input, parser_options)
        .map_err(|error| SanitizeError::Parse(error.to_string()))?;
    let mut report = sanitize_stylesheet_ast_with_options(&mut stylesheet, policy, options);
    report.record_parse_warnings(&warnings.read().unwrap_or_else(|error| error.into_inner()));
    let css = if stylesheet.rules.0.is_empty() {
        String::new()
    } else {
        stylesheet
            .to_css(PrinterOptions::default())
            .map_err(|error| SanitizeError::Serialize(error.to_string()))?
            .code
    };
    validate_output_size(&css, limits)?;
    Ok(SanitizeOutput {
        css: SanitizedCss::new(css),
        report,
    })
}
