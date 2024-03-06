use std::process::Command;
use std::str::from_utf8;

const GIT_COMMAND: &str = "git";
const GIT_ARGS: [&str; 3] = ["show", "--no-patch", "--format=%h %as"];

const DEFAULT_HASH: &str = "0000000";
const DEFAULT_DATE: &str = "0000-00-00";

fn main() {
    let output = Command::new(GIT_COMMAND).args(GIT_ARGS).output();

    let (hash, date) = match output {
        Ok(out) if out.status.success() => match from_utf8(&out.stdout) {
            Ok(s) => match s.split_once(' ') {
                Some((hash, date)) => (Some(hash.to_string()), Some(date.to_string())),
                None => (None, None),
            },
            Err(e) => {
                eprintln!("error parsing UTF-8 output: {}", e);
                (None, None)
            }
        },
        Ok(out) => {
            eprintln!("[{}]: {}", format_command(), out.status);
            (None, None)
        }
        Err(e) => {
            eprintln!("[{}]: {}", format_command(), e);
            (None, None)
        }
    };

    println!(
        "cargo:rustc-env=PED_VERSION_HASH={}",
        hash.unwrap_or(DEFAULT_HASH.to_string())
    );
    println!(
        "cargo:rustc-env=PED_VERSION_DATE={}",
        date.unwrap_or(DEFAULT_DATE.to_string())
    );
}

fn format_command() -> String {
    format!(
        "{GIT_COMMAND} {}",
        GIT_ARGS.iter().fold(String::new(), |mut args, a| {
            if args.len() > 0 {
                args.push(' ');
            }
            args.push_str(a);
            args
        })
    )
}
