#![forbid(unsafe_code)]

mod bundle;
mod dist;
#[cfg(feature = "hid")]
mod hid;
mod resign;

use std::process::ExitCode;

const USAGE: &str = "usage: cargo xtask bundle | dist --version X.Y.Z[-rc.N] | resign --identity <name> [--notarize] | hid <move x y | click x y | rightclick x y | drag x1 y1 x2 y2 | keys <combo>>";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("bundle") => bundle::run(),
        Some("dist") => dist::run(&args[1..]),
        Some("resign") => resign::run(&args[1..]),
        #[cfg(feature = "hid")]
        Some("hid") => hid::run(&args[1..]),
        _ => Err(USAGE.into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}
