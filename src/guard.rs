//! Engine-enforced value guard.
//!
//! After the structural policy decides to keep a declaration or descriptor, the
//! engine runs [`ValueGuard`] over it using lightningcss's own [`Visit`]
//! traversal. Because every AST node derives `Visit`, this reaches *every*
//! `url()`, `var()`, `env()`, and raw token nested anywhere inside the value —
//! including inside `@font-face` `src`, `image-set()`, `var()` fallbacks, and
//! tokens recovered from malformed input. Each value is checked against the
//! policy's `check_*` hooks (which are deny-by-default), so value-level
//! exfiltration cannot leak even if the structural policy forgot a hook.

use lightningcss::properties::Property;
use lightningcss::properties::custom::{EnvironmentVariable, TokenList, TokenOrValue, Variable};
use lightningcss::rules::font_face::FontFaceProperty;
use lightningcss::rules::font_palette_values::FontPaletteValuesProperty;
use lightningcss::rules::view_transition::ViewTransitionProperty;
use lightningcss::values::image::Image;
use lightningcss::values::syntax::ParsedComponent;
use lightningcss::values::url::Url;
use lightningcss::visitor::{Visit, VisitTypes, Visitor};

use crate::policy::{CssSanitizationPolicy, ValueAction, ValueContext};

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
