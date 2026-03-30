use std::process::Command;

const CRATE: &str = "css-sanitizer";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cmd = args.first().map(|s| s.as_str()).unwrap_or("help");

    match cmd {
        "publish" => publish(false),
        "publish-dry" => publish(true),
        "help" | "--help" | "-h" => print_help(),
        other => {
            eprintln!("Unknown command: {other}");
            print_help();
            std::process::exit(1);
        }
    }
}

fn publish(dry_run: bool) {
    let label = if dry_run { "Dry-run" } else { "Publishing" };
    println!("{label} {CRATE}...");

    let mut cmd = Command::new("cargo");
    cmd.args(["publish", "-p", CRATE]);
    if dry_run {
        cmd.arg("--dry-run");
    }

    let status = cmd.status().expect("failed to run cargo publish");
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    println!("Done.");
}

fn print_help() {
    println!(
        "\
Usage: cargo xtask <command>

Commands:
  publish       Publish {CRATE} to crates.io
  publish-dry   Dry-run publish (validate without uploading)
  help          Show this help message"
    );
}
