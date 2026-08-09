//! Differential completeness test.
//!
//! lightningcss's own `Visit` traversal is generated from its AST definitions.
//! We use it as an *oracle*: for a corpus of CSS, the
//! number of selector lists and the set of urls that lightningcss's traversal
//! reaches must equal what the sanitizer's policy is given a chance to inspect.
//! This verifies typed AST traversal for constructs present in the corpus;
//! semantic resources that lightningcss leaves generic are tested separately in
//! `security_audit_tests`.

mod common;

use std::cell::{Cell, RefCell};
use std::convert::Infallible;

use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::lightningcss::values::image::Image;
use css_sanitizer::lightningcss::values::url::Url;
use css_sanitizer::lightningcss::visitor::{Visit, VisitTypes, Visitor};
use css_sanitizer::{
    CssSanitizationPolicy, NodeAction, RuleContext, SelectorContext, ValueAction, ValueContext,
    sanitize_stylesheet_ast,
};

const CORPUS: &str = r#"
    .a, .b { color: red; background: url('bg.png') }
    @media (min-width: 1px) { .c { color: blue } }
    @keyframes spin { from { opacity: 0 } to { opacity: 1 } }
    @font-face { font-family: x; src: url('font.woff2') }
    @supports (display: grid) { .d { color: green } }
    @scope (.root .here) to (.limit) { .e { background-image: url('scope.png') } }
    .f { & .nested { color: pink } }
    @page { margin: 1cm; @top-left { content: url('mark.png') } }
    @container (min-width: 1px) { .g { color: gold } }
    @layer base { .h { color: tan } }
    @starting-style { .i { color: cyan } }
    @position-try --fallback { background-image: url('position.png') }
    .j { background-image: image-set(url('a.png') 1x, url('b.png') 2x) }
    .k { width: var(--x, url('var.png')) }
"#;

// ---- Oracle: lightningcss Visit ----

struct Oracle {
    selector_lists: usize,
    urls: Vec<String>,
}

impl<'i> Visitor<'i> for Oracle {
    type Error = Infallible;

    fn visit_types(&self) -> VisitTypes {
        VisitTypes::SELECTORS | VisitTypes::URLS | VisitTypes::IMAGES
    }

    fn visit_selector_list(
        &mut self,
        _selectors: &mut css_sanitizer::lightningcss::selector::SelectorList<'i>,
    ) -> Result<(), Infallible> {
        self.selector_lists += 1;
        Ok(())
    }

    fn visit_url(&mut self, url: &mut Url<'i>) -> Result<(), Infallible> {
        self.urls.push(url.url.to_string());
        Ok(())
    }

    fn visit_image(&mut self, image: &mut Image<'i>) -> Result<(), Infallible> {
        // Mirror the guard's image-set workaround (the option image is
        // `#[skip_type]`), so the oracle sees image-set urls too.
        if let Image::ImageSet(image_set) = image {
            for option in &mut image_set.options {
                self.visit_image(&mut option.image)?;
            }
            return Ok(());
        }
        image.visit_children(self)
    }
}

// ---- Recorder: an allow-all sanitizer policy that records what it is shown ----

#[derive(Default)]
struct Recorder {
    selector_lists: Cell<usize>,
    urls: RefCell<Vec<String>>,
}

impl CssSanitizationPolicy for Recorder {
    fn visit_rule(
        &self,
        _rule: &mut css_sanitizer::lightningcss::rules::CssRule<'_>,
        _: RuleContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    fn visit_selector_list(
        &self,
        _selectors: &mut css_sanitizer::lightningcss::selector::SelectorList<'_>,
        _: SelectorContext,
    ) -> NodeAction {
        self.selector_lists.set(self.selector_lists.get() + 1);
        NodeAction::Continue
    }

    fn visit_property(
        &self,
        _property: &mut css_sanitizer::lightningcss::properties::Property<'_>,
        _: css_sanitizer::PropertyContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    fn visit_font_face_property(
        &self,
        _property: &mut css_sanitizer::lightningcss::rules::font_face::FontFaceProperty<'_>,
        _: css_sanitizer::DescriptorContext,
    ) -> NodeAction {
        NodeAction::Continue
    }

    fn check_url(&self, url: &Url<'_>, _: ValueContext) -> ValueAction {
        self.urls.borrow_mut().push(url.url.to_string());
        ValueAction::Allow
    }

    fn check_variable(
        &self,
        _v: &css_sanitizer::lightningcss::properties::custom::Variable<'_>,
        _: ValueContext,
    ) -> ValueAction {
        ValueAction::Allow
    }

    fn check_environment_variable(
        &self,
        _e: &css_sanitizer::lightningcss::properties::custom::EnvironmentVariable<'_>,
        _: ValueContext,
    ) -> ValueAction {
        ValueAction::Allow
    }

    fn check_token(
        &self,
        _t: &css_sanitizer::lightningcss::properties::custom::TokenOrValue<'_>,
        _: ValueContext,
    ) -> ValueAction {
        ValueAction::Allow
    }
}

fn sorted(mut v: Vec<String>) -> Vec<String> {
    v.sort();
    v
}

#[test]
fn sanitizer_reaches_every_url_and_selector_list_the_visit_oracle_does() {
    // Oracle pass over the untouched AST.
    let mut oracle_sheet =
        StyleSheet::parse(CORPUS, ParserOptions::default()).expect("corpus should parse");
    let mut oracle = Oracle {
        selector_lists: 0,
        urls: Vec::new(),
    };
    oracle_sheet.visit(&mut oracle).unwrap();

    // Sanitizer pass with an allow-all recording policy.
    let mut sheet =
        StyleSheet::parse(CORPUS, ParserOptions::default()).expect("corpus should parse");
    let recorder = Recorder::default();
    sanitize_stylesheet_ast(&mut sheet, &recorder);

    assert_eq!(
        recorder.selector_lists.get(),
        oracle.selector_lists,
        "sanitizer must inspect exactly the selector lists the Visit oracle reaches",
    );

    assert_eq!(
        sorted(recorder.urls.into_inner()),
        sorted(oracle.urls),
        "sanitizer must inspect every url the Visit oracle reaches",
    );
}
