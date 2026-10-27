use cli_command_kit::Mode;
use mp4_atom::{Encode, Header, Stco, Stsc, StscEntry, Stsz, StszSamples};
use std::{
    ffi::OsString,
    io::{self, Write},
    sync::atomic::AtomicBool,
};

fn atom(kind: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    Header {
        kind: (*kind).into(),
        size: Some(payload.len()),
    }
    .encode(&mut bytes)
    .unwrap();
    bytes.extend_from_slice(payload);
    bytes
}

fn source() -> Vec<u8> {
    let mut data = atom(b"ftyp", b"isom\0\0\0\0isom");
    let start = data.len() as u32 + 8;
    data.extend(atom(b"mdat", b"abc"));
    let mut table = Vec::new();
    Stsz {
        samples: StszSamples::Identical { count: 1, size: 3 },
    }
    .encode(&mut table)
    .unwrap();
    Stco {
        entries: vec![start],
    }
    .encode(&mut table)
    .unwrap();
    Stsc {
        entries: vec![StscEntry {
            first_chunk: 1,
            samples_per_chunk: 1,
            sample_description_index: 1,
        }],
    }
    .encode(&mut table)
    .unwrap();
    data.extend(atom(
        b"moov",
        &atom(
            b"trak",
            &atom(b"mdia", &atom(b"minf", &atom(b"stbl", &table))),
        ),
    ));
    data
}

fn run(args: &[&str], mode: Mode, cancel: bool) -> (u8, String, String) {
    let mut out = Vec::new();
    let mut err = Vec::new();
    let status = engine_stems_cli::entry(
        args.iter().map(OsString::from),
        &mut out,
        &mut err,
        mode,
        &AtomicBool::new(cancel),
    );
    (
        status,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[test]
fn metadata_help_and_usage_preserve_stream_contracts() {
    for mode in [Mode::Human, Mode::Agent] {
        let (status, out, err) = run(&["stems", "--version"], mode, false);
        assert_eq!((status, out.as_str(), err.as_str()), (0, "0.1.0\n", ""));
        for args in [
            vec!["stems", "--help"],
            vec!["stems", "file", "--help"],
            vec!["stems", "file", "encrypt", "--help"],
            vec!["stems", "help", "file", "decrypt"],
            vec!["stems", "skill", "--help"],
        ] {
            let (status, out, err) = run(&args, mode, false);
            assert_eq!(status, 0, "{err}");
            assert!(err.is_empty());
            if mode == Mode::Agent {
                assert!(out.starts_with(
                    "ALERT: Agents must read `AGENT=1 stems skill` before using this tool.\n"
                ));
            } else {
                assert!(!out.contains("ALERT:"));
            }
        }
        for args in [
            vec!["stems", "file", "encrypt"],
            vec!["stems", "skill", "extra"],
            vec!["stems", "--bad"],
            vec!["stems", "file", "decrypt", "a", "b", "c"],
        ] {
            let (status, out, err) = run(&args, mode, false);
            assert_eq!(status, 1);
            assert!(out.is_empty());
            assert!(err.contains("error:"));
        }
        let (status, out, err) = run(&["stems", "skill"], mode, false);
        assert_eq!(
            (status, out.as_str(), err.as_str()),
            (0, engine_stems_cli::SKILL_POINTER, "")
        );
    }
}

struct Broken;
impl Write for Broken {
    fn write(&mut self, _: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("output unavailable"))
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[test]
fn cancellation_runtime_errors_and_writer_failures_report_once() {
    for mode in [Mode::Human, Mode::Agent] {
        let (status, out, err) = run(
            &["stems", "file", "encrypt", "missing", "target"],
            mode,
            true,
        );
        assert_eq!(status, 130);
        assert!(out.is_empty());
        assert_eq!(err.lines().count(), 1);
        assert!(err.contains("conversion cancelled"));
        let (status, out, err) = run(
            &["stems", "file", "decrypt", "missing", "target"],
            mode,
            false,
        );
        assert_eq!(status, 1);
        assert!(out.is_empty());
        assert_eq!(err.lines().count(), 1);
        assert!(!err.contains("Usage:"));
        for args in [
            vec!["stems", "skill"],
            vec!["stems", "--version"],
            vec!["stems", "--help"],
        ] {
            let mut errors = Vec::new();
            let status = engine_stems_cli::entry(
                args.iter().map(OsString::from),
                &mut Broken,
                &mut errors,
                mode,
                &AtomicBool::new(false),
            );
            assert_eq!(status, 1);
            assert_eq!(String::from_utf8(errors).unwrap().lines().count(), 1);
        }
    }
}

#[test]
fn file_round_trip_atomic_errors_and_output_failures_are_observable() {
    for mode in [Mode::Human, Mode::Agent] {
        let dir = tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/.tmp")).unwrap();
        let input = dir.path().join("in.mp4");
        let encrypted = dir.path().join("out.stems");
        let decrypted = dir.path().join("out.mp4");
        let before = source();
        std::fs::write(&input, &before).unwrap();
        let invoke = |verb: &str,
                      from: &std::path::Path,
                      to: &std::path::Path,
                      output: &mut Vec<u8>| {
            let args = [
                OsString::from("stems"),
                "file".into(),
                verb.into(),
                from.as_os_str().into(),
                to.as_os_str().into(),
            ];
            let mut errors = Vec::new();
            let status =
                engine_stems_cli::entry(args, output, &mut errors, mode, &AtomicBool::new(false));
            (status, String::from_utf8(errors).unwrap())
        };
        let mut output = Vec::new();
        assert_eq!(
            invoke("encrypt", &input, &encrypted, &mut output),
            (0, String::new())
        );
        if mode == Mode::Agent {
            let value: serde_json::Value = serde_json::from_slice(&output).unwrap();
            assert_eq!(value["samples"], 1);
            assert_eq!(value["padding_bytes"], 13);
            assert_eq!(value["input"], cli_command_kit::display_path(&input));
            assert_eq!(value["output"], cli_command_kit::display_path(&encrypted));
        } else {
            assert!(
                String::from_utf8(output)
                    .unwrap()
                    .contains("Padding bytes added: 13")
            );
        }
        assert_eq!(
            invoke("decrypt", &encrypted, &decrypted, &mut Vec::new()),
            (0, String::new())
        );
        let recovered = std::fs::read(&decrypted).unwrap();
        assert_eq!(&recovered[28..31], b"abc");
        assert_eq!(
            engine_stems::decrypt_sample(&std::fs::read(&encrypted).unwrap()[28..44]).unwrap(),
            b"abc"
        );
        let (status, error) = invoke("decrypt", &input, &decrypted, &mut Vec::new());
        assert_eq!(status, 1);
        assert!(error.contains("invalid AES block length"));
        assert_eq!(std::fs::read(&decrypted).unwrap(), recovered);
        let args = [
            OsString::from("stems"),
            "file".into(),
            "encrypt".into(),
            input.as_os_str().into(),
            encrypted.as_os_str().into(),
        ];
        assert_eq!(
            engine_stems_cli::entry(
                args,
                &mut Broken,
                &mut Vec::new(),
                mode,
                &AtomicBool::new(false)
            ),
            1
        );
        assert!(encrypted.is_file());
        assert_eq!(std::fs::read(&input).unwrap(), before);
    }
}

#[test]
fn native_binary_uses_environment_and_prints_raw_version() {
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_stems"))
        .args(["--version"])
        .env("AGENT", "1")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(result.stdout, b"0.1.0\n");
    assert!(result.stderr.is_empty());
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_stems"))
        .args(["--help"])
        .env("AGENT", "yes")
        .output()
        .unwrap();
    assert!(result.status.success());
    assert!(result.stdout.starts_with(b"ALERT:"));
}

#[test]
fn skill_covers_live_command_tree_and_local_install_destination() {
    let guide = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/skills/stems/SKILL.md"
    ))
    .unwrap();
    assert!(guide.lines().any(|line| line == "name: stems"));
    let missing = cli_command_kit::skill_omissions(
        &engine_stems_cli::command(),
        &engine_stems_cli::help_options(),
        &guide,
    );
    assert!(missing.is_empty(), "{missing:?}");
    let pointer = engine_stems_cli::SKILL_POINTER;
    assert!(pointer.contains("npx skills add . --skill stems\n"));
    let relative = pointer.lines().last().unwrap().trim();
    assert_eq!(relative, "skills/stems/SKILL.md");
    assert!(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(relative)
            .is_file()
    );
    let human = run(&["stems", "skill"], Mode::Human, false);
    let agent = run(&["stems", "skill"], Mode::Agent, false);
    assert_eq!(human, agent);
}

#[cfg(target_os = "linux")]
#[test]
fn native_sigint_cancels_after_blocking_input_without_publishing() {
    use std::os::unix::fs::OpenOptionsExt;
    let dir = tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/.tmp")).unwrap();
    let fifo = dir.path().join("input.mp4");
    let output = dir.path().join("target.stems");
    std::fs::write(&output, b"existing").unwrap();
    rustix::fs::mkfifoat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::Mode::RUSR | rustix::fs::Mode::WUSR,
    )
    .unwrap();
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_stems"))
        .args(["file", "encrypt"])
        .arg(&fifo)
        .arg(&output)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    let mut writer = loop {
        match std::fs::OpenOptions::new()
            .write(true)
            .custom_flags(rustix::fs::OFlags::NONBLOCK.bits() as i32)
            .open(&fifo)
        {
            Ok(writer) => break writer,
            Err(error) => {
                if std::time::Instant::now() >= deadline || child.try_wait().unwrap().is_some() {
                    let _ = child.kill();
                    let _ = child.wait();
                    panic!("child did not open the FIFO: {error}");
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let state = std::fs::read_to_string(format!("/proc/{}/wchan", child.id())).unwrap();
        if state.trim() == "pipe_read" {
            break;
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child did not enter the input read: {state}");
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    rustix::process::kill_process(
        rustix::process::Pid::from_child(&child),
        rustix::process::Signal::INT,
    )
    .unwrap();
    writer.write_all(&source()).unwrap();
    drop(writer);
    let result = child.wait_with_output().unwrap();
    assert_eq!(result.status.code(), Some(130));
    assert!(result.stdout.is_empty());
    let errors = String::from_utf8(result.stderr).unwrap();
    assert_eq!(errors.lines().count(), 1);
    assert!(errors.contains("conversion cancelled"));
    assert_eq!(std::fs::read(&output).unwrap(), b"existing");
}
