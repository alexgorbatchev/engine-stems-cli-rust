use anyhow::Result;
use cli_command_kit::{
    HelpOptions, Mode,
    clap::{Arg, ArgAction, ArgMatches, Command, value_parser},
    display_path,
};
use std::{ffi::OsString, io::Write, path::PathBuf, sync::atomic::AtomicBool};

pub const SKILL_POINTER: &str = "Install the stems skill using the Skills CLI from the local engine-stems-cli project directory:\n\n  npx skills add . --skill stems\n\nSkill:\n\n  skills/stems/SKILL.md\n";

pub fn command() -> Command {
    let operation = |name| {
        Command::new(name)
            .about(if name == "encrypt" {
                "Encrypt the first track's audio samples"
            } else {
                "Decrypt the first track's audio samples"
            })
            .arg(
                Arg::new("INPUT")
                    .required(true)
                    .value_parser(value_parser!(PathBuf))
                    .help("Source MP4 file"),
            )
            .arg(
                Arg::new("OUTPUT")
                    .required(true)
                    .value_parser(value_parser!(PathBuf))
                    .help("Destination file, replaced on success"),
            )
    };
    Command::new("stems")
        .about("Convert Engine DJ stems encryption in MP4 files")
        .arg_required_else_help(true)
        .arg(
            Arg::new("version")
                .long("version")
                .short('v')
                .action(ArgAction::SetTrue)
                .exclusive(true)
                .help("Print the raw version"),
        )
        .subcommand(
            Command::new("file")
                .about("Convert a local audio container")
                .subcommand_required(true)
                .subcommand(operation("encrypt"))
                .subcommand(operation("decrypt")),
        )
}

pub fn help_options() -> HelpOptions {
    HelpOptions {
        skill: Some(SKILL_POINTER.into()),
        ..Default::default()
    }
}

fn perform(
    matches: &ArgMatches,
    options: &HelpOptions,
    output: &mut impl Write,
    mode: Mode,
    cancel: &AtomicBool,
) -> Result<()> {
    if cli_command_kit::write_skill(matches, options, output)? {
        return Ok(());
    }
    if matches.get_flag("version") {
        writeln!(output, "{}", env!("CARGO_PKG_VERSION"))?;
        return Ok(());
    }
    let parent = matches
        .subcommand_matches("file")
        .expect("required file command");
    let (name, conversion) = parent.subcommand().expect("required conversion");
    let input = conversion.get_one::<PathBuf>("INPUT").unwrap();
    let destination = conversion.get_one::<PathBuf>("OUTPUT").unwrap();
    let operation = if name == "encrypt" {
        engine_stems::Operation::Encrypt
    } else {
        engine_stems::Operation::Decrypt
    };
    let stats = engine_stems::transform_file(input, destination, operation, cancel)?;
    if mode == Mode::Agent {
        serde_json::to_writer(
            &mut *output,
            &serde_json::json!({"operation":name,"input":display_path(input),"output":display_path(destination),"samples":stats.samples,"chunks":stats.chunks,"input_bytes":stats.input_bytes,"output_bytes":stats.output_bytes,"padding_bytes":stats.padding_bytes}),
        )?;
        writeln!(output)?;
    } else {
        writeln!(
            output,
            "[OK] {} -> {}\nSamples: {}\nChunks: {}\nBytes: {} -> {}\nPadding bytes {}: {}",
            display_path(input),
            display_path(destination),
            stats.samples,
            stats.chunks,
            stats.input_bytes,
            stats.output_bytes,
            if name == "encrypt" {
                "added"
            } else {
                "removed"
            },
            stats.padding_bytes
        )?;
    }
    Ok(())
}

pub fn entry(
    args: impl IntoIterator<Item = OsString>,
    output: &mut impl Write,
    diagnostics: &mut impl Write,
    mode: Mode,
    cancel: &AtomicBool,
) -> u8 {
    let options = help_options();
    let root =
        cli_command_kit::setup(command(), mode, &options).expect("valid static command tree");
    let result = match root.try_get_matches_from(args) {
        Ok(matches) => perform(&matches, &options, output, mode, cancel),
        Err(error) if !error.use_stderr() => output
            .write_all(error.to_string().as_bytes())
            .map_err(Into::into),
        Err(error) => {
            let _ = write!(diagnostics, "{error}");
            return 1;
        }
    };
    match result {
        Ok(()) => 0,
        Err(error) => {
            let _ = writeln!(
                diagnostics,
                "{} {error:#}",
                if mode == Mode::Agent {
                    "ERR:"
                } else {
                    "[ERROR]"
                }
            );
            if error.is::<engine_stems::Cancelled>() {
                130
            } else {
                1
            }
        }
    }
}
