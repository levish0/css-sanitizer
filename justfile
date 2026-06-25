set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-Command"]

# Default command to list all available commands.
default:
    @just --list

# Format all code
fmt:
    cargo fmt --all

# Run clippy with warnings denied
clippy:
    cargo clippy --all-targets -- -D warnings

# Run the test suite (e.g. `just test oracle`)
test *args:
    cargo test --workspace {{args}}

# Run all CI checks locally (fmt, clippy, tests)
check:
    cargo fmt --all --check
    cargo clippy --all-targets -- -D warnings
    cargo test --workspace

# Dry-run publish (validate without uploading)
publish-dry:
    cargo xtask publish-dry

# Publish css-sanitizer to crates.io
publish:
    cargo xtask publish
