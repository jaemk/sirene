use std::process::ExitCode;

fn main() -> ExitCode {
    match xtask::run(std::env::args_os(), &mut std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{}", e.trim_end());
            ExitCode::FAILURE
        }
    }
}
