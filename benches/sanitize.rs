use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use css_sanitizer::{
    ResourceUse, RuleKind, StrictPolicy, sanitize_declaration_list, sanitize_stylesheet,
};

fn sanitize_benchmarks(criterion: &mut Criterion) {
    let declaration_policy = StrictPolicy::new()
        .allow_properties(&[
            "color",
            "background-color",
            "background-image",
            "display",
            "padding",
            "margin",
        ])
        .allow_resources(&[ResourceUse::Image])
        .allow_variables();

    criterion.bench_function("sanitize declaration list", |bencher| {
        bencher.iter(|| {
            sanitize_declaration_list(
                black_box(
                    "color: red; display: grid; padding: 1rem; background-image: url('image.png'); position: fixed",
                ),
                black_box(&declaration_policy),
            )
            .expect("benchmark input should sanitize")
        });
    });

    let stylesheet_policy = declaration_policy
        .clone()
        .allow_unscoped_selectors()
        .allow_rules(&[
            RuleKind::Media,
            RuleKind::Supports,
            RuleKind::Container,
            RuleKind::LayerBlock,
        ]);

    criterion.bench_function("sanitize stylesheet", |bencher| {
        bencher.iter(|| {
            sanitize_stylesheet(
                black_box(
                    "@media (width > 30rem) { .card:hover { color: red; padding: 1rem; background-image: url('image.png') } }",
                ),
                black_box(&stylesheet_policy),
            )
            .expect("benchmark input should sanitize")
        });
    });
}

criterion_group!(benches, sanitize_benchmarks);
criterion_main!(benches);
