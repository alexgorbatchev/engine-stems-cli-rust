use engine_stems::{Operation, decrypt_sample, encrypt_sample, transform, transform_file};
use mp4_atom::{Co64, Decode, Encode, Header, Stco, Stsc, StscEntry, Stsz, StszSamples};
use std::sync::atomic::{AtomicBool, Ordering};

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

fn encoded(value: &impl Encode) -> Vec<u8> {
    let mut bytes = Vec::new();
    value.encode(&mut bytes).unwrap();
    bytes
}

fn first_track_samples(data: &[u8]) -> Vec<Vec<u8>> {
    let nodes =
        mp4box::get_boxes(&mut std::io::Cursor::new(data), data.len() as u64, false).unwrap();
    let mut node = nodes.iter().find(|b| b.typ == "moov").unwrap();
    for kind in ["trak", "mdia", "minf", "stbl"] {
        node = node
            .children
            .as_ref()
            .unwrap()
            .iter()
            .find(|b| b.typ == kind)
            .unwrap();
    }
    let items = node.children.as_ref().unwrap();
    let bytes = |kind: &str| {
        let item = items.iter().find(|b| b.typ == kind).unwrap();
        &data[item.offset as usize..(item.offset + item.size) as usize]
    };
    let sizes = match Stsz::decode(&mut bytes("stsz")).unwrap().samples {
        StszSamples::Identical { count, size } => vec![size; count as usize],
        StszSamples::Different { sizes } => sizes,
    };
    let offsets = if items.iter().any(|b| b.typ == "co64") {
        Co64::decode(&mut bytes("co64")).unwrap().entries
    } else {
        Stco::decode(&mut bytes("stco"))
            .unwrap()
            .entries
            .into_iter()
            .map(u64::from)
            .collect()
    };
    let mappings = Stsc::decode(&mut bytes("stsc")).unwrap().entries;
    let mut result = Vec::new();
    for (index, offset) in offsets.iter().enumerate() {
        let mapping = mappings
            .iter()
            .rev()
            .find(|m| m.first_chunk as usize <= index + 1)
            .unwrap();
        let mut cursor = *offset as usize;
        for _ in 0..mapping.samples_per_chunk {
            let size = sizes[result.len()] as usize;
            result.push(data[cursor..cursor + size].to_vec());
            cursor += size;
        }
    }
    result
}

fn track(samples: &[&[u8]], offsets: Vec<u64>, constant: bool, wide: bool) -> Vec<u8> {
    let size = if constant {
        StszSamples::Identical {
            count: samples.len() as u32,
            size: samples[0].len() as u32,
        }
    } else {
        StszSamples::Different {
            sizes: samples.iter().map(|s| s.len() as u32).collect(),
        }
    };
    let chunks = if wide {
        encoded(&Co64 { entries: offsets })
    } else {
        encoded(&Stco {
            entries: offsets.into_iter().map(|n| n as u32).collect(),
        })
    };
    let mut table = encoded(&Stsz { samples: size });
    table.extend(chunks);
    table.extend(encoded(&Stsc {
        entries: vec![StscEntry {
            first_chunk: 1,
            samples_per_chunk: 1,
            sample_description_index: 1,
        }],
    }));
    table.extend(atom(b"zzzz", b"untouched track extension"));
    atom(
        b"trak",
        &atom(b"mdia", &atom(b"minf", &atom(b"stbl", &table))),
    )
}

fn fixture(fast: bool, constant: bool, wide: bool, extended: bool, other_track: bool) -> Vec<u8> {
    let ftyp = atom(b"ftyp", b"M4A \0\0\0\0isom");
    let samples: [&[u8]; 2] = [b"first1234567890", b"second123456789"];
    let other = b"other-track-preserved";
    let make_moov = |base: u64| {
        let mut tracks = track(
            &samples,
            vec![
                base + 3,
                base + 3
                    + samples[0].len() as u64
                    + if other_track { other.len() as u64 } else { 0 },
            ],
            constant,
            wide,
        );
        if other_track {
            tracks.extend(track(
                &[other],
                vec![base + 3 + samples[0].len() as u64],
                false,
                !wide,
            ));
        }
        tracks.extend(atom(b"udta", &atom(b"zzzz", b"metadata preserved")));
        atom(b"moov", &tracks)
    };
    let moov_len = make_moov(0).len();
    let base =
        (ftyp.len() + if fast { moov_len } else { 0 } + if extended { 16 } else { 8 }) as u64;
    let moov = make_moov(base);
    let mut payload = b"gap".to_vec();
    payload.extend(samples[0]);
    if other_track {
        payload.extend(other);
    }
    payload.extend(samples[1]);
    payload.extend(b"tail");
    let mdat = if extended {
        let mut bytes = 1u32.to_be_bytes().to_vec();
        bytes.extend(b"mdat");
        bytes.extend(((payload.len() + 16) as u64).to_be_bytes());
        bytes.extend(payload);
        bytes
    } else {
        atom(b"mdat", &payload)
    };
    let mut bytes = ftyp;
    if fast {
        bytes.extend_from_slice(&moov);
    }
    bytes.extend(mdat);
    if !fast {
        bytes.extend(moov);
    }
    bytes.extend(atom(b"free", b""));
    bytes.extend(b"pad");
    bytes
}

#[test]
fn native_cipher_matches_verified_engine_block_and_rejects_bad_ciphertext() {
    let priming = [
        0xde, 0x04, 0x00, 0x4c, 0x61, 0x76, 0x63, 0x35, 0x38, 0x2e, 0x31, 0x33, 0x34, 0x2e, 0x31,
        0x30,
    ];
    let cipher = encrypt_sample(&priming);
    assert_eq!(
        &cipher[..16],
        &[
            0xe4, 0x85, 0xb0, 0x14, 0x11, 0x7a, 0xe5, 0x3b, 0xeb, 0x96, 0xde, 0xd1, 0xeb, 0x9e,
            0x4a, 0x51
        ]
    );
    for size in 0..=65 {
        let plain = vec![size as u8; size];
        assert_eq!(decrypt_sample(&encrypt_sample(&plain)).unwrap(), plain);
    }
    for bad in [vec![], vec![1], vec![0; 16], vec![1; 17]] {
        assert!(decrypt_sample(&bad).is_err());
    }
}

#[test]
fn round_trip_preserves_gaps_metadata_other_tracks_and_each_layout() {
    for fast in [false, true] {
        for constant in [false, true] {
            for wide in [false, true] {
                for extended in [false, true] {
                    for other in [false, true] {
                        let input = fixture(fast, constant, wide, extended, other);
                        let cancel = AtomicBool::new(false);
                        let encrypted = transform(&input, Operation::Encrypt, &cancel).unwrap();
                        assert_eq!(encrypted.stats.samples, 2);
                        assert_eq!(encrypted.stats.chunks, 2);
                        assert_eq!(encrypted.stats.padding_bytes, 2);
                        assert_eq!(
                            first_track_samples(&encrypted.bytes),
                            first_track_samples(&input)
                                .iter()
                                .map(|s| encrypt_sample(s))
                                .collect::<Vec<_>>()
                        );
                        let decrypted =
                            transform(&encrypted.bytes, Operation::Decrypt, &cancel).unwrap();
                        // The editor normalizes changed mdat/table headers; all sample bytes,
                        // unselected track bytes, gap/tail bytes, and opaque metadata survive.
                        let boxes = mp4box::get_boxes(
                            &mut std::io::Cursor::new(&decrypted.bytes),
                            decrypted.bytes.len() as u64,
                            false,
                        )
                        .unwrap();
                        assert_eq!(
                            first_track_samples(&decrypted.bytes),
                            first_track_samples(&input)
                        );
                        let mdat = boxes.iter().find(|b| b.typ == "mdat").unwrap();
                        let start = (mdat.offset + mdat.header_size) as usize;
                        let end = (mdat.offset + mdat.size) as usize;
                        let original_boxes = mp4box::get_boxes(
                            &mut std::io::Cursor::new(&input),
                            input.len() as u64,
                            false,
                        )
                        .unwrap();
                        let original_mdat =
                            original_boxes.iter().find(|b| b.typ == "mdat").unwrap();
                        assert_eq!(
                            &decrypted.bytes[start..end],
                            &input[(original_mdat.offset + original_mdat.header_size) as usize
                                ..(original_mdat.offset + original_mdat.size) as usize]
                        );
                        for marker in [
                            b"metadata preserved".as_slice(),
                            b"untouched track extension",
                            b"pad",
                        ] {
                            assert!(decrypted.bytes.windows(marker.len()).any(|v| v == marker));
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn malformed_tables_ranges_and_unsupported_offsets_fail_without_panics() {
    let cancel = AtomicBool::new(false);
    let valid = fixture(true, false, false, false, true);
    for length in 0..valid.len() {
        let short = &valid[..length];
        let result = std::panic::catch_unwind(|| transform(short, Operation::Encrypt, &cancel));
        assert!(result.is_ok(), "panic at length {length}");
    }
    for kind in [b"stsz", b"stco", b"stsc"] {
        let mut bad = valid.clone();
        let at = bad.windows(4).position(|v| v == kind).unwrap();
        bad[at + 8..at + 12].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(transform(&bad, Operation::Encrypt, &cancel).is_err());
    }
    let mut oversized = valid.clone();
    oversized[..4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(transform(&oversized, Operation::Encrypt, &cancel).is_err());
    for kind in [b"moof", b"sidx", b"saio", b"iloc"] {
        let mut bad = valid.clone();
        bad.extend(atom(kind, b"\0\0\0\0"));
        assert!(transform(&bad, Operation::Encrypt, &cancel).is_err());
    }
}

#[test]
fn multiple_media_boxes_and_changing_chunk_counts_retain_exact_samples() {
    let samples: [&[u8]; 3] = [b"a", b"bb", b"ccc"];
    let mut input = atom(b"ftyp", b"isom\0\0\0\0isom");
    let first = input.len() as u64 + 8;
    input.extend(atom(b"mdat", b"abb"));
    input.extend(atom(b"free", b"gap"));
    let second = input.len() as u64 + 8;
    input.extend(atom(b"mdat", b"ccc"));
    input.extend(atom(
        b"moov",
        &track(&samples, vec![first, second], false, true),
    ));
    let mut editor = mp4box::edit::Editor::new();
    editor.add_command(mp4box::edit::Command::Replace {
        path: "moov/trak/mdia/minf/stbl/stsc".into(),
        bytes: encoded(&Stsc {
            entries: vec![
                StscEntry {
                    first_chunk: 1,
                    samples_per_chunk: 2,
                    sample_description_index: 1,
                },
                StscEntry {
                    first_chunk: 2,
                    samples_per_chunk: 1,
                    sample_description_index: 1,
                },
            ],
        }),
    });
    let mut fixture = Vec::new();
    editor
        .process(&mut std::io::Cursor::new(input), &mut fixture)
        .unwrap();
    let cancel = AtomicBool::new(false);
    let encrypted = transform(&fixture, Operation::Encrypt, &cancel).unwrap();
    assert_eq!(encrypted.stats.samples, 3);
    assert_eq!(encrypted.stats.chunks, 2);
    assert_eq!(
        first_track_samples(&encrypted.bytes),
        samples
            .iter()
            .map(|s| encrypt_sample(s))
            .collect::<Vec<_>>()
    );
    let decrypted = transform(&encrypted.bytes, Operation::Decrypt, &cancel).unwrap();
    assert_eq!(first_track_samples(&decrypted.bytes), samples);
}

#[test]
fn to_eof_media_and_invalid_sample_mappings_are_handled_explicitly() {
    let cancel = AtomicBool::new(false);
    let mut input = fixture(true, false, false, false, false);
    let nodes =
        mp4box::get_boxes(&mut std::io::Cursor::new(&input), input.len() as u64, false).unwrap();
    let mdat = nodes.iter().find(|b| b.typ == "mdat").unwrap();
    input.truncate((mdat.offset + mdat.size) as usize);
    input[mdat.offset as usize..mdat.offset as usize + 4].fill(0);
    let encrypted = transform(&input, Operation::Encrypt, &cancel).unwrap();
    assert_eq!(
        first_track_samples(&encrypted.bytes),
        first_track_samples(&input)
            .iter()
            .map(|s| encrypt_sample(s))
            .collect::<Vec<_>>()
    );
    for (delta, value) in [(12, 0u32), (16, 0), (16, 3), (20, 0)] {
        let mut bad = input.clone();
        let at = bad.windows(4).position(|v| v == b"stsc").unwrap() - 4;
        bad[at + delta..at + delta + 4].copy_from_slice(&value.to_be_bytes());
        assert!(transform(&bad, Operation::Encrypt, &cancel).is_err());
    }
    let mut overlap = fixture(true, false, false, false, true);
    let at = overlap.windows(4).position(|v| v == b"stco").unwrap() - 4;
    let first = overlap[at + 16..at + 20].to_vec();
    overlap[at + 20..at + 24].copy_from_slice(&first);
    assert!(transform(&overlap, Operation::Encrypt, &cancel).is_err());
}

#[test]
fn atomic_file_conversion_preserves_source_and_old_target_on_errors_or_cancel() {
    let dir = tempfile::tempdir_in(concat!(env!("CARGO_MANIFEST_DIR"), "/.tmp")).unwrap();
    let input = dir.path().join("input.m4a");
    let output = dir.path().join("output.stems");
    let plain = fixture(false, false, false, false, false);
    std::fs::write(&input, &plain).unwrap();
    std::fs::write(&output, b"old target").unwrap();
    let cancel = AtomicBool::new(true);
    let failure = transform_file(&input, &output, Operation::Encrypt, &cancel).unwrap_err();
    assert!(failure.is::<engine_stems::Cancelled>());
    assert!(
        transform(&plain, Operation::Encrypt, &cancel)
            .unwrap_err()
            .is::<engine_stems::Cancelled>()
    );
    assert_eq!(std::fs::read(&output).unwrap(), b"old target");
    cancel.store(false, Ordering::Relaxed);
    assert!(transform_file(&input, &input, Operation::Encrypt, &cancel).is_err());
    let link = dir.path().join("hardlink");
    std::fs::hard_link(&input, &link).unwrap();
    assert!(transform_file(&input, &link, Operation::Encrypt, &cancel).is_err());
    transform_file(&input, &output, Operation::Encrypt, &cancel).unwrap();
    assert_eq!(std::fs::read(&input).unwrap(), plain);
    assert!(
        transform_file(
            &output,
            &dir.path().join("missing/target"),
            Operation::Decrypt,
            &cancel
        )
        .is_err()
    );
    std::fs::write(&input, b"invalid").unwrap();
    let saved = std::fs::read(&output).unwrap();
    assert!(transform_file(&input, &output, Operation::Decrypt, &cancel).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), saved);
}

#[test]
#[ignore = "explicit local MP4 input required through ENGINE_STEMS_REFERENCE"]
fn native_reference_preserves_every_sample_after_file_round_trip() {
    let input = std::path::PathBuf::from(std::env::var_os("ENGINE_STEMS_REFERENCE").unwrap());
    let output = std::path::PathBuf::from(std::env::var_os("ENGINE_STEMS_EVIDENCE").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let encrypted = output.join("encrypted.stems");
    let decrypted = output.join("decrypted.mp4");
    let cancel = AtomicBool::new(false);
    let before = std::fs::read(&input).unwrap();
    let encoded = transform_file(&input, &encrypted, Operation::Encrypt, &cancel).unwrap();
    let decoded = transform_file(&encrypted, &decrypted, Operation::Decrypt, &cancel).unwrap();
    assert_eq!(encoded.samples, decoded.samples);
    assert_eq!(encoded.chunks, decoded.chunks);
    assert_eq!(std::fs::read(&input).unwrap(), before);
    assert_eq!(
        first_track_samples(&std::fs::read(&decrypted).unwrap()),
        first_track_samples(&before)
    );
    eprintln!(
        "verified {} samples in {} chunks; {} bytes padding",
        encoded.samples, encoded.chunks, encoded.padding_bytes
    );
}
