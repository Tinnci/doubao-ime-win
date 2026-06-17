use std::path::PathBuf;
use std::process::ExitCode;

#[cfg(windows)]
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(not(windows))]
fn main() -> ExitCode {
    eprintln!("doubao-tip-tool only supports Windows.");
    ExitCode::FAILURE
}

#[cfg(windows)]
fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "status".to_string());

    match command.as_str() {
        "register" => {
            let dll_path = parse_dll_path(args)?;
            let dll_path = dll_path.canonicalize().map_err(|error| {
                format!("cannot resolve DLL path {}: {error}", dll_path.display())
            })?;
            doubao_tsf_tip::register_server_with_path(&dll_path.to_string_lossy())
                .map_err(|error| format!("{error:?}"))?;
            println!("registered {}", dll_path.display());
        }
        "unregister" => {
            doubao_tsf_tip::unregister_server().map_err(|error| format!("{error:?}"))?;
            println!("unregistered Doubao TSF TIP");
        }
        "status" => {
            print_status();
        }
        "help" | "--help" | "-h" => {
            print_help();
        }
        other => {
            return Err(format!(
                "unknown command '{other}'. Run 'doubao-tip-tool help'."
            ));
        }
    }

    Ok(())
}

#[cfg(windows)]
fn parse_dll_path<I>(mut args: I) -> Result<PathBuf, String>
where
    I: Iterator<Item = String>,
{
    let mut dll_path = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--dll-path" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--dll-path requires a value".to_string())?;
                dll_path = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }

    dll_path.map_or_else(default_dll_path, Ok)
}

#[cfg(windows)]
fn default_dll_path() -> Result<PathBuf, String> {
    let exe =
        std::env::current_exe().map_err(|error| format!("cannot locate tool exe: {error}"))?;
    let dir = exe
        .parent()
        .ok_or_else(|| format!("cannot locate parent directory for {}", exe.display()))?;
    Ok(dir.join("doubao_tsf_tip.dll"))
}

#[cfg(windows)]
fn print_status() {
    let status = doubao_tsf_tip::query_registration_status();

    println!("Doubao TSF TIP registration status");
    println!("  description: {}", status.description);
    println!("  clsid: {}", status.clsid);
    println!("  profile: {}", status.profile_guid);
    println!("  langid: 0x{:04x}", status.langid);
    println!("  COM key present: {}", yes_no(status.com_key_present));
    println!(
        "  COM DLL path: {}",
        status.com_dll_path.as_deref().unwrap_or("<missing>")
    );
    println!(
        "  threading model: {}",
        status.threading_model.as_deref().unwrap_or("<missing>")
    );
    println!(
        "  TSF profile registry key present: {}",
        yes_no(status.tsf_profile_key_present)
    );
    println!(
        "  TSF profile registered: {}",
        yes_no(status.tsf_profile_registered)
    );
    match status.tsf_profile_enabled {
        Some(enabled) => println!("  TSF profile enabled: {}", yes_no(enabled)),
        None if !status.tsf_profile_registered => {
            println!("  TSF profile enabled: <not registered>")
        }
        None => println!(
            "  TSF profile enabled: <unknown> ({})",
            status
                .tsf_profile_error
                .as_deref()
                .unwrap_or("no diagnostic error")
        ),
    }
    println!(
        "  keyboard category registered: {}",
        yes_no(status.keyboard_category_registered)
    );
}

#[cfg(windows)]
fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(windows)]
fn print_help() {
    println!("Usage:");
    println!("  doubao-tip-tool status");
    println!("  doubao-tip-tool register [--dll-path <path-to-doubao_tsf_tip.dll>]");
    println!("  doubao-tip-tool unregister");
}
