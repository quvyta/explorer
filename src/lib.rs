//! qexp: a file explorer for the terminal, with an icon for every kind of file, opening with the
//! right program and archives extracted with a right click.

pub mod app;
pub mod archive;
pub mod cli;
pub mod favourites;
pub mod locales;
pub mod places;
mod programs;

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;

use qframe::prelude::*;

use app::{Machine, Opening};
use cli::Invocation;

/// Runs one `qexp` invocation on this machine. Both commands, `qexp` and `quvyta-explorer`, start
/// here.
///
/// # Errors
///
/// Returns the terminal's error when the screen cannot be opened or drawn.
pub fn run() -> io::Result<ExitCode> {
    let machine = Machine::here();
    let cwd = std::env::current_dir().unwrap_or_else(|_| machine.home.clone());
    let invocation = cli::parse(std::env::args_os().skip(1), &machine.home, &cwd);
    let start = match invocation {
        Invocation::Screen(start) => start,
        other => return Ok(answer(&other)),
    };
    let config = machine.config.clone();
    let opening = Opening::new(machine, start);
    let runtime = locales::LOCALES
        .iter()
        .fold(Runtime::new(opening.explorer), |runtime, (file, text)| runtime.locale_source(*file, *text))
        .keymap_source(locales::KEYMAP.0, locales::KEYMAP.1)
        .settings(&opening.settings)
        .preferences(&opening.preferences);
    // A member follows the look another Quvyta application changes while qexp is open; the
    // settings and preferences given above are used as they are and not read twice.
    let runtime = match config {
        Some(folder) => runtime.member_in(qframe::storage::Ecosystem::QUVYTA, folder, app::APP),
        None => runtime,
    };
    runtime.run()?;
    Ok(ExitCode::SUCCESS)
}

/// Says what a command line that opens no screen asks for, in the person's language, and gives the
/// exit code: 0 for the version and the help, 2 for a path that is not there or an unknown option.
#[must_use]
pub fn answer(invocation: &Invocation) -> ExitCode {
    let env = locales::env();
    let detected = env.i18n().detect(|name| std::env::var(name).ok());
    let mut i18n = env.i18n().clone();
    if let Some(code) = detected {
        i18n.set_active(&code);
    }
    qframe::i18n::scope(Arc::new(i18n), || {
        let mut out = io::stdout().lock();
        match invocation {
            Invocation::Screen(_) => ExitCode::SUCCESS,
            Invocation::Version => {
                let _ = writeln!(out, "qexp {}", env!("CARGO_PKG_VERSION"));
                ExitCode::SUCCESS
            }
            Invocation::Help => {
                let _ = writeln!(out, "{}", t!("explorer.cli.help", version = env!("CARGO_PKG_VERSION")));
                ExitCode::SUCCESS
            }
            Invocation::Missing(path) => {
                eprintln!("{}", t!("explorer.cli.missing", path = shown(path)));
                ExitCode::from(2)
            }
            Invocation::Unknown(argument) => {
                eprintln!("{}", t!("explorer.cli.unknown", argument = argument.as_str()));
                ExitCode::from(2)
            }
        }
    })
}

/// A path as the message shows it.
fn shown(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_that_is_not_there_or_an_unknown_option_exits_with_2_and_the_rest_with_0() {
        assert_eq!(answer(&Invocation::Missing("/nowhere/at/all".into())), ExitCode::from(2));
        assert_eq!(answer(&Invocation::Unknown("--frobnicate".into())), ExitCode::from(2));
        assert_eq!(answer(&Invocation::Version), ExitCode::SUCCESS);
        assert_eq!(answer(&Invocation::Help), ExitCode::SUCCESS);
    }
}
