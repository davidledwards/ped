use std::error::Error;
use std::process::{Command, ExitCode};
use std::str;

const GIT_COMMAND: [&str; 4] = ["git", "show", "--no-patch", "--format=%h %as"];

const BUILD_HASH: &str = "cargo:rustc-env=BUILD_HASH";
const BUILD_DATE: &str = "cargo:rustc-env=BUILD_DATE";

fn main() -> ExitCode {
    match run() {
        Err(e) => {
            eprintln!("`{}`: {e}", GIT_COMMAND.join(" "));
            ExitCode::from(1)
        }
        Ok(code) => code,
    }
}

fn run() -> Result<ExitCode, Box<dyn Error>> {
    let out = Command::new(GIT_COMMAND[0])
        .args(&GIT_COMMAND[1..])
        .output()?;
    let code = if out.status.success() {
        let s = str::from_utf8(&out.stdout)?;
        if let Some((hash, date)) = s.split_once(' ') {
            println!("{BUILD_HASH}={hash}");
            println!("{BUILD_DATE}={date}");
            ExitCode::SUCCESS
        } else {
            eprintln!("\"{s}\": unable to split string");
            ExitCode::from(1)
        }
    } else {
        eprintln!(
            "`{}`: exited with status {}",
            GIT_COMMAND.join(" "),
            out.status
        );
        eprintln!("{}", str::from_utf8(&out.stderr)?);
        ExitCode::from(1)
    };
    Ok(code)
}
