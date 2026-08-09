//! Engine-enforced value guard.
//!
//! After the structural policy decides to keep a declaration or descriptor, the
//! engine runs [`ValueGuard`] over it using lightningcss's own [`Visit`]
//! traversal. Typed URLs, images, variables, environment variables, and raw
//! tokens are visited recursively; generic resource functions that upstream
//! leaves unparsed (`src()`, string-based `image()`/`image-set()`) are recognized
//! explicitly. Each value is checked against the policy's deny-by-default
//! `check_*` hooks, so value-level exfiltration cannot leak merely because a
//! structural policy forgot a hook.

use lightningcss::properties::Property;
use lightningcss::properties::custom::{
    EnvironmentVariable, Function, Token, TokenList, TokenOrValue, Variable,
};
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::values::image::Image;
use lightningcss::values::syntax::ParsedComponent;
use lightningcss::values::url::Url;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};

use crate::policy::{CssSanitizationPolicy, ResourceKind, ResourceRef, ValueAction, ValueContext};

/// Sentinel error used to short-circuit the `Visit` traversal as soon as a value
/// is denied. The `?` operator unwinds the entire `node.visit(..)` call.
struct Denied;

struct ValueGuard<'a> {
    policy: &'a dyn CssSanitizationPolicy,
    ctx: ValueContext,
}

impl<'a> ValueGuard<'a> {
    fn act(action: ValueAction) -> Result<(), Denied> {
        match action {
            ValueAction::Allow => Ok(()),
            ValueAction::Deny => Err(Denied),
        }
    }

    fn check_resource(&self, kind: ResourceKind, value: Option<&str>) -> Result<(), Denied> {
        let resource = match value {
            Some(value) => ResourceRef::literal(kind, value),
            None => ResourceRef::dynamic(kind),
        };
        Self::act(self.policy.check_resource(resource, self.ctx))
    }

    fn check_function_resources(
        &self,
        function: &Function<'_>,
        kind: ResourceKind,
        always_resource: bool,
    ) -> Result<(), Denied> {
        let mut found_resource = false;
        let mut found_dynamic = false;

        for argument in &function.arguments.0 {
            match argument {
                TokenOrValue::Token(Token::String(value)) => {
                    found_resource = true;
                    self.check_resource(kind, Some(value.as_ref()))?;
                }
                argument if Self::is_unresolved_resource_candidate(argument) => {
                    found_dynamic = true;
                }
                _ => {}
            }
        }

        if found_dynamic || (always_resource && !found_resource) {
            self.check_resource(kind, None)?;
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

    fn visit_generic_function(&mut self, function: &mut Function<'_>) -> Result<(), Denied> {
        let name = function.name.0.as_ref();

        if name.eq_ignore_ascii_case("url") {
            // lightningcss's raw TokenList parser only upgrades exactly
            // lower-case `url` to TokenOrValue::Url. CSS function names are
            // ASCII-insensitive, so upper-case and escaped spellings can still
            // arrive here and must retain URL semantics.
            self.check_function_resources(function, ResourceKind::Url, true)?;
        } else if name.eq_ignore_ascii_case("var") {
            Self::act(self.policy.check_unparsed_variable(function, self.ctx))?;
        } else if name.eq_ignore_ascii_case("env") {
            Self::act(
                self.policy
                    .check_unparsed_environment_variable(function, self.ctx),
            )?;
        } else if name.eq_ignore_ascii_case("src") {
            self.check_function_resources(function, ResourceKind::Src, true)?;
        } else if name.eq_ignore_ascii_case("image") {
            self.check_function_resources(function, ResourceKind::Image, false)?;
        } else if name.eq_ignore_ascii_case("image-set")
            || name.eq_ignore_ascii_case("-webkit-image-set")
        {
            self.check_function_resources(function, ResourceKind::ImageSet, false)?;
        } else {
            Self::act(self.policy.check_function(function, self.ctx))?;
        }

        function.visit_children(self)
    }
}

impl<'i, 'a> Visitor<'i> for ValueGuard<'a> {
    type Error = Denied;

    fn visit_types(&self) -> VisitTypes {
        VisitTypes::URLS
            | VisitTypes::IMAGES
            | VisitTypes::VARIABLES
            | VisitTypes::ENVIRONMENT_VARIABLES
            | VisitTypes::TOKENS
    }

    fn visit_url(&mut self, url: &mut Url<'i>) -> Result<(), Denied> {
        Self::act(self.policy.check_url(url, self.ctx))
    }

    fn visit_image(&mut self, image: &mut Image<'i>) -> Result<(), Denied> {
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

    fn visit_variable(&mut self, variable: &mut Variable<'i>) -> Result<(), Denied> {
        Self::act(self.policy.check_variable(variable, self.ctx))?;
        variable.visit_children(self)
    }

    fn visit_environment_variable(
        &mut self,
        env: &mut EnvironmentVariable<'i>,
    ) -> Result<(), Denied> {
        Self::act(self.policy.check_environment_variable(env, self.ctx))?;
        env.visit_children(self)
    }

    fn visit_token(&mut self, token: &mut TokenOrValue<'i>) -> Result<(), Denied> {
        // When `TOKENS` is requested, lightningcss dispatches *every*
        // `TokenOrValue` here first — including the parsed `Url`/`Var`/`Env`
        // variants — so we must route those to their dedicated checks before
        // falling back to `check_token` for genuinely raw/unknown tokens.
        match token {
            TokenOrValue::Url(url) => self.visit_url(url),
            TokenOrValue::Var(variable) => {
                Self::act(self.policy.check_variable(variable, self.ctx))?;
                variable.visit_children(self)
            }
            TokenOrValue::Env(env) => {
                Self::act(self.policy.check_environment_variable(env, self.ctx))?;
                env.visit_children(self)
            }
            TokenOrValue::Function(function) => self.visit_generic_function(function),
            TokenOrValue::Token(Token::UnquotedUrl(value)) => {
                self.check_resource(ResourceKind::Url, Some(value.as_ref()))
            }
            // A bad URL is malformed by definition. Never let error recovery
            // turn it into a policy-controlled fetchable value.
            TokenOrValue::Token(Token::BadUrl(_)) => Err(Denied),
            other => {
                Self::act(self.policy.check_token(other, self.ctx))?;
                other.visit_children(self)
            }
        }
    }
}

/// Returns `true` if every value inside `property` is allowed by the policy.
pub(crate) fn property_allowed(
    property: &mut Property<'_>,
    policy: &dyn CssSanitizationPolicy,
    ctx: ValueContext,
) -> bool {
    let mut guard = ValueGuard { policy, ctx };
    property.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside an `@font-face` descriptor is allowed.
pub(crate) fn font_face_property_allowed(
    property: &mut FontFaceProperty<'_>,
    policy: &dyn CssSanitizationPolicy,
    ctx: ValueContext,
) -> bool {
    let mut guard = ValueGuard { policy, ctx };
    property.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside an `@font-palette-values` descriptor is
/// allowed.
pub(crate) fn font_palette_values_property_allowed(
    property: &mut FontPaletteValuesProperty<'_>,
    policy: &dyn CssSanitizationPolicy,
    ctx: ValueContext,
) -> bool {
    let mut guard = ValueGuard { policy, ctx };
    property.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside a `@view-transition` descriptor is
/// allowed.
pub(crate) fn view_transition_property_allowed(
    property: &mut ViewTransitionProperty<'_>,
    policy: &dyn CssSanitizationPolicy,
    ctx: ValueContext,
) -> bool {
    let mut guard = ValueGuard { policy, ctx };
    property.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside a raw token list is allowed.
pub(crate) fn token_list_allowed(
    tokens: &mut TokenList<'_>,
    policy: &dyn CssSanitizationPolicy,
    ctx: ValueContext,
) -> bool {
    let mut guard = ValueGuard { policy, ctx };
    tokens.visit(&mut guard).is_ok()
}

/// Returns `true` if every value inside an `@property` `initial-value` is
/// allowed. `ParsedComponent::Repeated.components` is `#[skip_type]` in
/// lightningcss, so the repeated case is recursed manually.
pub(crate) fn parsed_component_allowed(
    component: &mut ParsedComponent<'_>,
    policy: &dyn CssSanitizationPolicy,
    ctx: ValueContext,
) -> bool {
    if let ParsedComponent::Repeated { components, .. } = component {
        return components
            .iter_mut()
            .all(|component| parsed_component_allowed(component, policy, ctx));
    }

    let mut guard = ValueGuard { policy, ctx };
    component.visit(&mut guard).is_ok()
}
