//! The `jumpgate` binary, which runs the command that the jumpgate-cli
//! library holds.

use std::process::ExitCode;

/// Runs the command and returns its exit code, so destructors run before
/// the process ends (BASE-7).
fn main() -> ExitCode {
    // A shell's completion request ends the run here. jumpgate parses
    // against `Cli::command()`, so completion offers what parsing accepts.
    if let Some(code) = crumbkit::complete(<jumpgate_cli::Cli as clap::CommandFactory>::command) {
        return code;
    }
    jumpgate_cli::run(std::env::args_os())
}
