//! Resource and dynamic-value validation after structural policy decisions.
//!
//! After the structural policy decides to keep a declaration or descriptor, the
//! engine runs [`ValueGuard`] over it using lightningcss's own [`Visit`]
//! traversal. Typed URLs, images, variables, environment variables, and raw
//! tokens are visited recursively; generic resource functions that upstream
//! leaves unparsed (`src()`, string-based `image()`/`image-set()`) are recognized
//! explicitly. Each value is checked against the policy's deny-by-default
//! resource, dynamic-value, and token hooks, so retaining a structural node does
//! not implicitly grant permissions to its values.

use cssparser::{Delimiter, Parser, ParserInput};
use lightningcss::properties::Property;
use lightningcss::properties::custom::{
    CustomProperty, EnvironmentVariable, Function, Token, TokenList, TokenOrValue, Variable,
};
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::stylesheet::ParserOptions;
use lightningcss::traits::ParseWithOptions;
use lightningcss::values::image::Image;
use lightningcss::values::string::CowArcStr;
use lightningcss::values::syntax::ParsedComponent;
use lightningcss::values::url::Url;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};

use crate::limits::enforce_input_limits;
use crate::options::SanitizeLimits;
use crate::policy::{
    CssPolicy, DynamicValueRef, PropertyLocation, ResourceRef, ResourceSyntax, ResourceUse,
    ValueContext, ValueDecision,
};

/// Sentinel error used to short-circuit the `Visit` traversal as soon as a value
/// is denied. The `?` operator unwinds the entire `node.visit(..)` call.
struct ValueRejected;

struct ValueGuard<'a, 'i> {
    policy: &'a dyn CssPolicy,
    value_context: ValueContext<'i>,
    limits: SanitizeLimits,
    checking_serialized_tokens: bool,
}

impl<'a, 'i> ValueGuard<'a, 'i> {
    fn new(
        policy: &'a dyn CssPolicy,
        value_context: ValueContext<'i>,
        limits: SanitizeLimits,
    ) -> Self {
        Self {
            policy,
            value_context,
            limits,
            checking_serialized_tokens: false,
        }
    }

    fn require_allowed(action: ValueDecision) -> Result<(), ValueRejected> {
        match action {
            ValueDecision::Allow => Ok(()),
            ValueDecision::Deny => Err(ValueRejected),
        }
    }

    fn check_resource(
        &self,
        syntax: ResourceSyntax,
        value: Option<&str>,
    ) -> Result<(), ValueRejected> {
        let use_kind = ResourceUse::from_value_context(&self.value_context);
        let resource = match value {
            Some(value) => ResourceRef::literal(syntax, use_kind, value),
            None => ResourceRef::dynamic(syntax, use_kind),
        };
        Self::require_allowed(self.policy.resource(resource, &self.value_context))
    }

    fn check_function_resources(
        &self,
        function: &Function<'_>,
        syntax: ResourceSyntax,
        always_resource: bool,
    ) -> Result<(), ValueRejected> {
        let mut found_resource = false;
        let mut found_dynamic = false;

        for argument in &function.arguments.0 {
            match argument {
                TokenOrValue::Token(Token::String(value)) => {
                    found_resource = true;
                    self.check_resource(syntax, Some(value.as_ref()))?;
                }
                argument if Self::is_unresolved_resource_candidate(argument) => {
                    found_dynamic = true;
                }
                _ => {}
            }
        }

        if found_dynamic || (always_resource && !found_resource) {
            self.check_resource(syntax, None)?;
        }

        Ok(())
    }

    fn is_unresolved_resource_candidate(value: &TokenOrValue<'_>) -> bool {
        // var()/env() and every still-generic function may substitute a value
        // at computed-value time. This includes CSS Values 5 if(), custom
        // dashed functions, and future arbitrary-substitution functions. When
        // nested in generic image()/image-set() syntax, fail closed unless the
        // resource policy explicitly permits a dynamic resource.
        matches!(
            value,
            TokenOrValue::Var(_) | TokenOrValue::Env(_) | TokenOrValue::Function(_)
        )
    }

    fn visit_generic_function(&mut self, function: &mut Function<'i>) -> Result<(), ValueRejected> {
        let name = function.name.0.as_ref();

        if name.eq_ignore_ascii_case("url") {
            // lightningcss's raw TokenList parser only upgrades exactly
            // lower-case `url` to TokenOrValue::Url. CSS function names are
            // ASCII-insensitive, so upper-case and escaped spellings can still
            // arrive here and must retain URL semantics.
            self.check_function_resources(function, ResourceSyntax::Url, true)?;
        } else if name.eq_ignore_ascii_case("var") {
            Self::require_allowed(self.policy.dynamic_value(
                DynamicValueRef::UnparsedVariable(function),
                &self.value_context,
            ))?;
        } else if name.eq_ignore_ascii_case("env") {
            Self::require_allowed(self.policy.dynamic_value(
                DynamicValueRef::UnparsedEnvironmentVariable(function),
                &self.value_context,
            ))?;
        } else if name.eq_ignore_ascii_case("src") {
            self.check_function_resources(function, ResourceSyntax::Src, true)?;
        } else if name.eq_ignore_ascii_case("image") {
            self.check_function_resources(function, ResourceSyntax::Image, false)?;
        } else if name.eq_ignore_ascii_case("image-set")
            || name.eq_ignore_ascii_case("-webkit-image-set")
        {
            self.check_function_resources(function, ResourceSyntax::ImageSet, false)?;
        } else {
            Self::require_allowed(
                self.policy
                    .dynamic_value(DynamicValueRef::Function(function), &self.value_context),
            )?;
        }

        function.visit_children(self)
    }
}

impl<'i, 'a> Visitor<'i> for ValueGuard<'a, 'i> {
    type Error = ValueRejected;

    fn visit_types(&self) -> VisitTypes {
        VisitTypes::URLS
            | VisitTypes::IMAGES
            | VisitTypes::VARIABLES
            | VisitTypes::ENVIRONMENT_VARIABLES
            | VisitTypes::TOKENS
    }

    fn visit_url(&mut self, url: &mut Url<'i>) -> Result<(), ValueRejected> {
        self.check_resource(ResourceSyntax::Url, Some(url.url.as_ref()))
    }

    fn visit_image(&mut self, image: &mut Image<'i>) -> Result<(), ValueRejected> {
        // `ImageSetOption::image` is marked `#[skip_type]` in lightningcss, so it
        // is excluded from `CHILD_TYPES` and the derive's type-pruning would skip
        // the entire `image-set()` contents. Recurse into the options manually so
        // urls smuggled through `image-set(url(...))` are still checked.
        if let Image::ImageSet(image_set) = image {
            for option in &mut image_set.options {
                self.visit_image(&mut option.image)?;
            }
            return Ok(());
        }

        image.visit_children(self)
    }

    fn visit_variable(&mut self, variable: &mut Variable<'i>) -> Result<(), ValueRejected> {
        Self::require_allowed(
            self.policy
                .dynamic_value(DynamicValueRef::Variable(variable), &self.value_context),
        )?;
        variable.visit_children(self)
    }

    fn visit_environment_variable(
        &mut self,
        environment_variable: &mut EnvironmentVariable<'i>,
    ) -> Result<(), ValueRejected> {
        Self::require_allowed(self.policy.dynamic_value(
            DynamicValueRef::EnvironmentVariable(environment_variable),
            &self.value_context,
        ))?;
        environment_variable.visit_children(self)
    }

    fn visit_token_list(&mut self, tokens: &mut TokenList<'i>) -> Result<(), ValueRejected> {
        if self.checking_serialized_tokens {
            return tokens.visit_children(self);
        }

        // Callers can construct or rewrite AST token lists. Separate raw tokens
        // such as Ident("url") + ParenthesisBlock may print as a single function.
        // Check the emitted syntax, not just the individual pre-print tokens.
        if tokens.0.iter().any(|token| {
            matches!(
                token,
                TokenOrValue::Token(Token::BadUrl(_) | Token::BadString(_) | Token::Function(_))
            )
        }) {
            return Err(ValueRejected);
        }
        let value = Property::Custom(CustomProperty {
            name: CowArcStr::from("--sanitizer-value").into(),
            value: tokens.clone(),
        })
        .value_to_css_string(Default::default())
        .map_err(|_| ValueRejected)?;
        enforce_input_limits(&value, self.limits).map_err(|_| ValueRejected)?;

        let mut input = ParserInput::new(&value);
        let mut parser = Parser::new(&mut input);
        let options = ParserOptions::default();
        let mut serialized_tokens =
            if self.value_context.location == PropertyLocation::OpaqueAtRule {
                TokenList::parse_with_options(&mut parser, &options)
            } else {
                // A fabricated top-level semicolon or !important must not escape
                // the declaration policy when this value is printed.
                parser.parse_until_before(Delimiter::Semicolon | Delimiter::Bang, |parser| {
                    TokenList::parse_with_options(parser, &options)
                })
            }
            .map_err(|_| ValueRejected)?;
        parser.expect_exhausted().map_err(|_| ValueRejected)?;

        let mut guard = ValueGuard {
            policy: self.policy,
            value_context: self.value_context.clone(),
            limits: self.limits,
            checking_serialized_tokens: true,
        };
        serialized_tokens.visit_children(&mut guard)
    }

    fn visit_token(&mut self, token: &mut TokenOrValue<'i>) -> Result<(), ValueRejected> {
        // When `TOKENS` is requested, lightningcss dispatches *every*
        // `TokenOrValue` here first — including the parsed `Url`/`Var`/`Env`
        // variants — so we must route those to their dedicated checks before
        // falling back to the token hook for genuinely raw/unknown tokens.
        match token {
            TokenOrValue::Url(url) => self.visit_url(url),
            TokenOrValue::Var(variable) => self.visit_variable(variable),
            TokenOrValue::Env(environment_variable) => {
                self.visit_environment_variable(environment_variable)
            }
            TokenOrValue::Function(function) => self.visit_generic_function(function),
            TokenOrValue::Token(Token::UnquotedUrl(value)) => {
                self.check_resource(ResourceSyntax::Url, Some(value.as_ref()))
            }
            // A bad URL is malformed by definition. Never let error recovery
            // turn it into a policy-controlled fetchable value.
            TokenOrValue::Token(Token::BadUrl(_) | Token::BadString(_) | Token::Function(_)) => {
                Err(ValueRejected)
            }
            other => {
                Self::require_allowed(self.policy.token(other, &self.value_context))?;
                other.visit_children(self)
            }
        }
    }
}

/// Returns `true` if every value inside `property` is allowed by the policy.
pub(crate) fn property_allowed<'i>(
    property: &mut Property<'i>,
    policy: &dyn CssPolicy,
    value_context: ValueContext<'i>,
    limits: SanitizeLimits,
) -> bool {
    let mut guard = ValueGuard::new(policy, value_context, limits);
    property.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside an `@font-face` descriptor is allowed.
pub(crate) fn font_face_property_allowed<'i>(
    property: &mut FontFaceProperty<'i>,
    policy: &dyn CssPolicy,
    value_context: ValueContext<'i>,
    limits: SanitizeLimits,
) -> bool {
    let mut guard = ValueGuard::new(policy, value_context, limits);
    property.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside an `@font-palette-values` descriptor is
/// allowed.
pub(crate) fn font_palette_values_property_allowed<'i>(
    property: &mut FontPaletteValuesProperty<'i>,
    policy: &dyn CssPolicy,
    value_context: ValueContext<'i>,
    limits: SanitizeLimits,
) -> bool {
    let mut guard = ValueGuard::new(policy, value_context, limits);
    property.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside a `@view-transition` descriptor is
/// allowed.
pub(crate) fn view_transition_property_allowed<'i>(
    property: &mut ViewTransitionProperty<'i>,
    policy: &dyn CssPolicy,
    value_context: ValueContext<'i>,
    limits: SanitizeLimits,
) -> bool {
    let mut guard = ValueGuard::new(policy, value_context, limits);
    property.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside a raw token list is allowed.
pub(crate) fn token_list_allowed<'i>(
    tokens: &mut TokenList<'i>,
    policy: &dyn CssPolicy,
    value_context: ValueContext<'i>,
    limits: SanitizeLimits,
) -> bool {
    let mut guard = ValueGuard::new(policy, value_context, limits);
    tokens.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside an `@property` `initial-value` is
/// allowed. `ParsedComponent::Repeated.components` is `#[skip_type]` in
/// lightningcss, so the repeated case is recursed manually.
pub(crate) fn parsed_component_allowed<'i>(
    component: &mut ParsedComponent<'i>,
    policy: &dyn CssPolicy,
    value_context: ValueContext<'i>,
    limits: SanitizeLimits,
) -> bool {
    if let ParsedComponent::Repeated { components, .. } = component {
        return components.iter_mut().all(|component| {
            parsed_component_allowed(component, policy, value_context.clone(), limits)
        });
    }

    let mut guard = ValueGuard::new(policy, value_context, limits);
    component.visit(&mut guard).is_ok()
}
