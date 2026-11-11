#![forbid(unsafe_code)]

mod bundle;
mod hid;

use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = match args.first().map(String::as_str) {
        Some("bundle") => bundle::run(),
        Some("hid") => hid::run(&args[1..]),
        _ => Err("usage: cargo xtask bundle | hid <move x y | click x y | rightclick x y | drag x1 y1 x2 y2 | keys <combo>>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("xtask: {message}");
            ExitCode::FAILURE
        }
    }
}
