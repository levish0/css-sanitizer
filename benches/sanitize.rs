use std::fmt::Write;
use std::hint::black_box;
use std::time::Duration;

use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use css_sanitizer::lightningcss::declaration::DeclarationBlock;
use css_sanitizer::lightningcss::printer::PrinterOptions;
use css_sanitizer::lightningcss::properties::Property;
use css_sanitizer::lightningcss::rules::CssRule;
use css_sanitizer::lightningcss::rules::font_face::FontFaceProperty;
use css_sanitizer::lightningcss::stylesheet::{ParserOptions, StyleSheet};
use css_sanitizer::lightningcss::traits::ToCss;
use css_sanitizer::{
    CssSanitizationPolicy, DescriptorContext, NodeAction, PropertyContext, RuleContext,
    clean_declaration_list_with_policy, clean_stylesheet_with_policy, sanitize_stylesheet_ast,
};

struct Fixture {
    name: &'static str,
    css: String,
}

struct PassThroughPolicy;

impl CssSanitizationPolicy for PassThroughPolicy {}

struct FilteringPolicy;

impl FilteringPolicy {
    fn property_css(property: &Property<'_>, important: bool) -> String {
        property
            .to_css_string(important, PrinterOptions::default())
            .unwrap_or_default()
            .to_ascii_lowercase()
    }

    fn rule_allowed(rule: &CssRule<'_>) -> bool {
        matches!(
            rule,
            CssRule::Style(_)
                | CssRule::Media(_)
                | CssRule::Keyframes(_)
                | CssRule::FontFace(_)
                | CssRule::FontPaletteValues(_)
                | CssRule::FontFeatureValues(_)
                | CssRule::Page(_)
                | CssRule::Supports(_)
                | CssRule::Nesting(_)
                | CssRule::NestedDeclarations(_)
                | CssRule::Viewport(_)
                | CssRule::LayerBlock(_)
                | CssRule::Container(_)
                | CssRule::Scope(_)
                | CssRule::StartingStyle(_)
                | CssRule::ViewTransition(_)
        )
    }

    fn property_allowed(name: &str) -> bool {
        matches!(
            name,
            "color"
                | "background-color"
                | "width"
                | "padding"
                | "margin"
                | "animation"
                | "transform"
                | "display"
                | "gap"
                | "grid-template-columns"
                | "opacity"
                | "zoom"
                | "font-weight"
                | "font-style"
        )
    }
}

impl CssSanitizationPolicy for FilteringPolicy {
    fn visit_rule(&self, rule: &mut CssRule<'_>, _ctx: RuleContext) -> NodeAction {
        if Self::rule_allowed(rule) {
            NodeAction::Continue
        } else {
            NodeAction::Drop
        }
    }

    fn visit_property(&self, property: &mut Property<'_>, ctx: PropertyContext) -> NodeAction {
        if ctx.important {
            return NodeAction::Drop;
        }

        let property_id = property.property_id();
        let name = property_id.name();
        if !Self::property_allowed(name) {
            return NodeAction::Drop;
        }

        let css = Self::property_css(property, false);
        if css.contains("url(") || css.contains("expression(") {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }

    fn visit_font_face_property(
        &self,
        property: &mut FontFaceProperty<'_>,
        _ctx: DescriptorContext,
    ) -> NodeAction {
        if matches!(property, FontFaceProperty::Source(_)) {
            NodeAction::Drop
        } else {
            NodeAction::Continue
        }
    }
}

fn benchmark_config() -> Criterion {
    Criterion::default()
        .sample_size(20)
        .warm_up_time(Duration::from_millis(500))
        .measurement_time(Duration::from_secs(2))
}

fn parser_options<'i>() -> ParserOptions<'static, 'i> {
    ParserOptions {
        error_recovery: true,
        ..ParserOptions::default()
    }
}

fn roundtrip_declaration_list(input: &str) -> String {
    let block =
        DeclarationBlock::parse_string(input, parser_options()).expect("declaration list parses");
    block
        .to_css_string(PrinterOptions::default())
        .expect("declaration list serializes")
}

fn roundtrip_stylesheet(input: &str) -> String {
    let stylesheet = StyleSheet::parse(input, parser_options()).expect("stylesheet parses");
    stylesheet
        .to_css(PrinterOptions::default())
        .expect("stylesheet serializes")
        .code
}

fn declaration_fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            name: "small",
            css: build_inline_fixture(6),
        },
        Fixture {
            name: "medium",
            css: build_inline_fixture(24),
        },
        Fixture {
            name: "large",
            css: build_inline_fixture(96),
        },
    ]
}

fn stylesheet_fixtures() -> Vec<Fixture> {
    vec![
        Fixture {
            name: "small",
            css: build_stylesheet_fixture(4),
        },
        Fixture {
            name: "medium",
            css: build_stylesheet_fixture(20),
        },
        Fixture {
            name: "large",
            css: build_stylesheet_fixture(80),
        },
    ]
}

fn build_inline_fixture(blocks: usize) -> String {
    let mut css = String::new();

    for i in 0..blocks {
        let offset = 8 + (i % 5) * 2;
        let hue = 180 + (i % 60);
        let width = 320 + (i % 7) * 24;

        let _ = write!(
            css,
            "\
            --theme-space-{i}: {offset}px; \
            color: hsl({hue} 70% 35%); \
            background-color: rgb({r}, {g}, {b}) !important; \
            width: min(100%, calc({width}px - 1rem)); \
            padding: var(--space-{i}, {offset}px); \
            margin: clamp(8px, 2vw, 24px); \
            background-image: image-set(url('https://cdn.example.com/{i}.png') 1x); \
            position: fixed; \
            transform: translate3d({x}px, {y}px, 0) scale(1); \
            ",
            r = 20 + (i % 10),
            g = 40 + (i % 20),
            b = 60 + (i % 30),
            x = i % 9,
            y = (i * 2) % 11,
        );
    }

    css
}

fn build_stylesheet_fixture(components: usize) -> String {
    let mut css = String::new();

    css.push_str(":root { --brand-hue: 210; --radius: 12px; }\n");
    css.push_str("@import url(\"https://evil.test/base.css\");\n");
    css.push_str(
        "@font-face { font-family: BenchSans; src: url(\"https://evil.test/bench.woff2\"); font-weight: 400; font-style: normal; }\n",
    );
    css.push_str(
        "@font-feature-values BenchSans { @styleset { alt-glyphs: 1 2; } @swash { fancy: 3; } }\n",
    );
    css.push_str("@font-palette-values --brand { base-palette: 1; override-colors: 0 red; }\n");
    css.push_str("@view-transition { navigation: auto; types: hero card; }\n");
    css.push_str(
        "@keyframes pulse { from { opacity: 0; transform: scale(.98); } to { opacity: 1; transform: scale(1); } }\n",
    );
    css.push_str(
        "@page { margin: 1cm; @top-left { color: red; background-image: url(\"https://evil.test/page.png\"); } }\n",
    );
    css.push_str("@viewport { zoom: 1; width: device-width; }\n");

    for i in 0..components {
        let hue = 180 + (i % 90);
        let width = 320 + (i % 12) * 24;
        let gap = 8 + (i % 5) * 2;
        let container = 360 + (i % 8) * 32;
        let media = 640 + (i % 6) * 48;

        let _ = writeln!(
            css,
            r#"
@layer components {{
  .card-{i}, .card-{i}[data-tone="warm"] > .title {{
    color: hsl({hue} 70% 35%);
    background-color: rgb({r}, {g}, {b});
    background-image: image-set(url("https://cdn.example.com/card-{i}.png") 1x);
    width: min(100%, calc({width}px - 1rem));
    padding: var(--space-{i}, {gap}px);
    margin: clamp(8px, 2vw, 24px);
    position: fixed;
    animation: pulse 180ms ease-out both;
    transform: translateY({shift}px);

    & .badge {{
      color: var(--accent-{i}, #06c);
      background-image: url("https://evil.test/nested-{i}.svg");
    }}
  }}

  @media (min-width: {media}px) {{
    .card-{i} {{
      display: grid;
      gap: {gap}px;
      background-image: url("https://evil.test/media-{i}.png");
    }}
  }}

  @supports (display: grid) {{
    .card-{i} {{
      display: grid;
      grid-template-columns: 1fr auto;
    }}
  }}

  @container card-{i} (min-width: {container}px) {{
    .card-{i} {{
      grid-template-columns: 2fr 1fr;
      background-image: url("https://evil.test/container-{i}.png");
    }}
  }}

  @scope (.card-{i}) {{
    .title {{
      color: rebeccapurple;
      background-image: url("https://evil.test/scope-{i}.png");
    }}
  }}

  @starting-style {{
    .card-{i} {{
      opacity: 0;
      transform: translateY(8px);
    }}
  }}
}}
"#,
            r = 20 + (i % 10),
            g = 40 + (i % 20),
            b = 60 + (i % 30),
            shift = i % 9,
        );
    }

    css
}

fn bench_declaration_lists(c: &mut Criterion) {
    let fixtures = declaration_fixtures();
    let pass_through = PassThroughPolicy;
    let filtering = FilteringPolicy;
    let mut group = c.benchmark_group("declaration_list/end_to_end");

    for fixture in &fixtures {
        group.throughput(Throughput::Bytes(fixture.css.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("lightningcss", fixture.name),
            fixture,
            |b, f| b.iter(|| black_box(roundtrip_declaration_list(black_box(f.css.as_str())))),
        );

        group.bench_with_input(
            BenchmarkId::new("pass_through", fixture.name),
            fixture,
            |b, f| {
                b.iter(|| {
                    black_box(clean_declaration_list_with_policy(
                        black_box(f.css.as_str()),
                        &pass_through,
                    ))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("filtering", fixture.name),
            fixture,
            |b, f| {
                b.iter(|| {
                    black_box(clean_declaration_list_with_policy(
                        black_box(f.css.as_str()),
                        &filtering,
                    ))
                })
            },
        );
    }

    group.finish();
}

fn bench_stylesheets(c: &mut Criterion) {
    let fixtures = stylesheet_fixtures();
    let pass_through = PassThroughPolicy;
    let filtering = FilteringPolicy;
    let mut group = c.benchmark_group("stylesheet/end_to_end");

    for fixture in &fixtures {
        group.throughput(Throughput::Bytes(fixture.css.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("lightningcss", fixture.name),
            fixture,
            |b, f| b.iter(|| black_box(roundtrip_stylesheet(black_box(f.css.as_str())))),
        );

        group.bench_with_input(
            BenchmarkId::new("pass_through", fixture.name),
            fixture,
            |b, f| {
                b.iter(|| {
                    black_box(clean_stylesheet_with_policy(
                        black_box(f.css.as_str()),
                        &pass_through,
                    ))
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("filtering", fixture.name),
            fixture,
            |b, f| {
                b.iter(|| {
                    black_box(clean_stylesheet_with_policy(
                        black_box(f.css.as_str()),
                        &filtering,
                    ))
                })
            },
        );
    }

    group.finish();
}

fn bench_stylesheet_ast_api(c: &mut Criterion) {
    let fixtures = stylesheet_fixtures();
    let pass_through = PassThroughPolicy;
    let filtering = FilteringPolicy;
    let mut group = c.benchmark_group("stylesheet/ast_api");

    for fixture in &fixtures {
        group.throughput(Throughput::Bytes(fixture.css.len() as u64));

        group.bench_with_input(
            BenchmarkId::new("pass_through", fixture.name),
            fixture,
            |b, f| {
                b.iter(|| {
                    let mut stylesheet =
                        StyleSheet::parse(black_box(f.css.as_str()), parser_options())
                            .expect("benchmark fixture should parse");
                    sanitize_stylesheet_ast(&mut stylesheet, &pass_through);
                    black_box(stylesheet.rules.0.len())
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("filtering", fixture.name),
            fixture,
            |b, f| {
                b.iter(|| {
                    let mut stylesheet =
                        StyleSheet::parse(black_box(f.css.as_str()), parser_options())
                            .expect("benchmark fixture should parse");
                    sanitize_stylesheet_ast(&mut stylesheet, &filtering);
                    black_box(stylesheet.rules.0.len())
                })
            },
        );
    }

    group.finish();
}

criterion_group!(
    name = benches;
    config = benchmark_config();
    targets = bench_declaration_lists, bench_stylesheets, bench_stylesheet_ast_api
);
criterion_main!(benches);
