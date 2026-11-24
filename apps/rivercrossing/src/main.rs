// SPDX-License-Identifier: GPL-3.0-only

//! The `RiverCrossing` desktop binary: open the shell window and run it until it closes.

use rivercrossing_ui_egui::ui::automation::ControlConfig;

#[cfg(debug_assertions)]
use std::io::Write;

#[cfg(debug_assertions)]
use std::path::PathBuf;

#[cfg(debug_assertions)]
use craft::automation::args::{
    ControlOptions, EnvNames, FlagNames, OptionsError, Source, USAGE_EXIT_CODE, control_port,
};

/// Open the `RiverCrossing` window and run the app until the window closes.
fn main() -> eframe::Result {
    rivercrossing_ui_egui::run(smoke_quit_after(), control_launch())
}

/// Return the debug-only self-quit delay from `RIVERCROSSING_SMOKE_QUIT_AFTER_MS`.
///
/// This is the launch-path smoke seam. The functional test sets the variable, so the window
/// closes by itself and the test never has to signal the process. Only a debug build reads it:
/// a release build ignores the variable, because a shipped app must not quit on environment
/// input.
#[cfg(debug_assertions)]
fn smoke_quit_after() -> Option<std::time::Duration> {
    std::env::var("RIVERCROSSING_SMOKE_QUIT_AFTER_MS")
        .ok()
        .and_then(|ms| ms.parse().ok())
        .map(std::time::Duration::from_millis)
}

/// Return no self-quit delay: a release build never quits on environment input.
#[cfg(not(debug_assertions))]
fn smoke_quit_after() -> Option<std::time::Duration> {
    None
}

/// Parse the debug-only control flags and resolve their environment fallbacks.
///
/// The whole acquisition is debug-only: a release build returns `None` and ignores both the flags
/// and the `RIVERCROSSING_CONTROL_*` variables.
#[cfg(debug_assertions)]
fn control_launch() -> Option<ControlConfig> {
    let names = EnvNames::for_app("RIVERCROSSING");
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let flags = parse_control_flags(&arguments);
    let options = ControlOptions::resolve(flags, &names).unwrap_or_else(|error| usage_exit(error));
    options
        .check(&FlagNames::default(), &names)
        .unwrap_or_else(|error| usage_exit(error));
    options.port?;
    let token = options
        .server_token()
        .unwrap_or_else(|error| usage_exit(error));
    notice(&options, &token);
    Some(ControlConfig { options, token })
}

/// Return no control configuration: a release build ignores the flags and the environment.
#[cfg(not(debug_assertions))]
fn control_launch() -> Option<ControlConfig> {
    None
}

/// Parse `--control`, `--control-token`, `--control-token-file` and `--help`.
#[cfg(debug_assertions)]
fn parse_control_flags(args: &[String]) -> ControlOptions {
    let mut options = ControlOptions::default();
    let mut index = 0;
    while let Some(argument) = args.get(index) {
        match argument.as_str() {
            "--help" | "-h" => help_exit(),
            "--control" => {
                let value = next_value(args, &mut index, "--control <PORT>");
                options.port = Some(control_port(&value).unwrap_or_else(|_| {
                    usage_exit(OptionsError::NotAPort {
                        value,
                        shown: "--control <PORT>".to_owned(),
                    })
                }));
                options.sources.port = Some(Source::Flag);
            }
            "--control-token" => {
                options.token = Some(next_value(args, &mut index, "--control-token <HEX>"));
                options.sources.token = Some(Source::Flag);
            }
            "--control-token-file" => {
                options.token_file = Some(PathBuf::from(next_value(
                    args,
                    &mut index,
                    "--control-token-file <PATH>",
                )));
                options.sources.token_file = Some(Source::Flag);
            }
            other => usage_exit(format!("unrecognised argument `{other}`")),
        }
        index = index.saturating_add(1);
    }
    options
}

/// Return the value after flag `what` in `args`, advancing `index` onto it.
#[cfg(debug_assertions)]
fn next_value(args: &[String], index: &mut usize, what: &str) -> String {
    *index = index.saturating_add(1);
    args.get(*index)
        .cloned()
        .unwrap_or_else(|| usage_exit(format!("missing value for {what}")))
}

/// Print a usage error with the `error: ` prefix and exit with the usage status.
#[cfg(debug_assertions)]
fn usage_exit(error: impl std::fmt::Display) -> ! {
    writeln!(std::io::stderr(), "error: {error}").ok();
    std::process::exit(USAGE_EXIT_CODE)
}

/// Print the control flags and exit successfully.
#[cfg(debug_assertions)]
fn help_exit() -> ! {
    writeln!(
        std::io::stdout(),
        "usage: rivercrossing [--control <PORT>] [--control-token <HEX> | --control-token-file <PATH>]"
    )
    .ok();
    writeln!(
        std::io::stdout(),
        "  --control <PORT>             listen on 127.0.0.1:<PORT> (0 picks a free port)"
    )
    .ok();
    writeln!(
        std::io::stdout(),
        "  --control-token <HEX>        authenticate with a 64-hex token"
    )
    .ok();
    writeln!(
        std::io::stdout(),
        "  --control-token-file <PATH>  read the 64-hex token from a file"
    )
    .ok();
    writeln!(
        std::io::stdout(),
        "  --help, -h                   print these flags and exit"
    )
    .ok();
    std::process::exit(0)
}

/// Print the token notice, so a generated token is discoverable.
#[cfg(debug_assertions)]
fn notice(options: &ControlOptions, token: &str) {
    writeln!(std::io::stderr(), "{}", options.token_notice(token)).ok();
}
