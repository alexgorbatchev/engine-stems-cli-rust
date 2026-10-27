use std::{
    io,
    sync::{Arc, atomic::AtomicBool},
};

fn main() {
    let cancel = Arc::new(AtomicBool::new(false));
    let mode = cli_command_kit::Mode::from_env();
    let registration =
        signal_hook::flag::register(signal_hook::consts::SIGINT, Arc::clone(&cancel));
    let status = match registration {
        Ok(id) => {
            let status = engine_stems_cli::entry(
                std::env::args_os(),
                &mut io::stdout().lock(),
                &mut io::stderr().lock(),
                mode,
                &cancel,
            );
            signal_hook::low_level::unregister(id);
            status
        }
        Err(error) => {
            use io::Write;
            let _ = writeln!(
                io::stderr().lock(),
                "{} installing interrupt handler: {error}",
                if mode == cli_command_kit::Mode::Agent {
                    "ERR:"
                } else {
                    "[ERROR]"
                }
            );
            1
        }
    };
    std::process::exit(i32::from(status));
}
