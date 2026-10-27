use crate::{check_cancel, decrypt_sample, encrypt_sample};
use anyhow::{Context, Result, bail, ensure};
use mp4_atom::{Atom, Co64, Decode, Encode, Mdat, Stco, Stsc, Stsz, StszSamples};
use mp4box::{
    BoxRef, NodeKind,
    edit::{Command, Editor},
};
use std::{io::Cursor, ops::Range, sync::atomic::AtomicBool};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Operation {
    Encrypt,
    Decrypt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Stats {
    pub samples: usize,
    pub chunks: usize,
    pub input_bytes: usize,
    pub output_bytes: usize,
    pub padding_bytes: usize,
}

#[derive(Debug)]
pub struct Converted {
    pub bytes: Vec<u8>,
    pub stats: Stats,
}

struct Table {
    path: String,
    wide: bool,
    chunks: Vec<u64>,
    samples: Vec<Range<usize>>,
}

struct Change {
    range: Range<usize>,
    bytes: Vec<u8>,
}

fn children(node: &BoxRef) -> &[BoxRef] {
    match &node.kind {
        NodeKind::Container(children) | NodeKind::FullContainer { children, .. } => children,
        _ => &[],
    }
}

fn unique<'a>(nodes: &'a [BoxRef], kind: &[u8; 4]) -> Result<&'a BoxRef> {
    let mut matches = nodes.iter().filter(|node| &node.hdr.typ.0 == kind);
    let node = matches
        .next()
        .context(format!("missing {} box", String::from_utf8_lossy(kind)))?;
    ensure!(
        matches.next().is_none(),
        "ambiguous duplicate {} boxes",
        String::from_utf8_lossy(kind)
    );
    Ok(node)
}

fn end(node: &BoxRef, parent_end: usize) -> Result<usize> {
    if node.hdr.size == 0 {
        return Ok(parent_end);
    }
    let end = node
        .hdr
        .start
        .checked_add(node.hdr.size)
        .context("box size overflow")?;
    ensure!(end <= parent_end as u64, "box extends beyond its parent");
    Ok(usize::try_from(end)?)
}

fn validate_geometry(nodes: &[BoxRef], parent_end: usize) -> Result<()> {
    for node in nodes {
        let node_end = end(node, parent_end)?;
        ensure!(
            node.hdr.start + node.hdr.header_size <= node_end as u64,
            "box header exceeds its parent"
        );
        ensure!(
            ![b"moof", b"sidx", b"iloc", b"saio", b"tfra", b"tfhd"].contains(&&node.hdr.typ.0),
            "unsupported offset-bearing box {}",
            node.hdr.typ
        );
        validate_geometry(children(node), node_end)?;
    }
    Ok(())
}

fn tree(data: &[u8]) -> Result<Vec<BoxRef>> {
    let nodes = mp4box::parse_boxes(&mut Cursor::new(data), 0, data.len() as u64)
        .context("parsing MP4 container")?;
    validate_geometry(&nodes, data.len())?;
    Ok(nodes)
}

fn decode<T: Atom + Decode>(node: &BoxRef, data: &[u8]) -> Result<T> {
    ensure!(node.hdr.size != 0, "sample table needs an explicit size");
    let mut bytes = &data[node.hdr.start as usize..end(node, data.len())?];
    let decoded = T::decode(&mut bytes).context("decoding sample table")?;
    ensure!(bytes.is_empty(), "trailing bytes in sample table");
    Ok(decoded)
}

fn parse_tables(nodes: &[BoxRef], data: &[u8], media: &[Range<usize>]) -> Result<Vec<Table>> {
    let moov = unique(nodes, b"moov")?;
    let mut tables = Vec::new();
    for (index, track) in children(moov)
        .iter()
        .filter(|n| &n.hdr.typ.0 == b"trak")
        .enumerate()
    {
        let mdia = unique(children(track), b"mdia")?;
        let minf = unique(children(mdia), b"minf")?;
        let stbl = unique(children(minf), b"stbl")?;
        let entries = children(stbl);
        ensure!(
            !entries.iter().any(|n| &n.hdr.typ.0 == b"stz2"),
            "compact sample-size tables are unsupported"
        );
        let sizes = decode::<Stsz>(unique(entries, b"stsz")?, data)?;
        let sizes = match sizes.samples {
            StszSamples::Identical { count, size } => {
                ensure!(
                    size > 0 && count as usize <= data.len(),
                    "invalid constant sample count or size"
                );
                vec![size; count as usize]
            }
            StszSamples::Different { sizes } => sizes,
        };
        ensure!(
            !sizes.is_empty() && sizes.iter().all(|s| *s > 0),
            "empty sample table or zero-sized sample"
        );
        let offset_nodes: Vec<_> = entries
            .iter()
            .filter(|n| [b"stco", b"co64"].contains(&&n.hdr.typ.0))
            .collect();
        ensure!(
            offset_nodes.len() == 1,
            "expected exactly one chunk-offset table"
        );
        let wide = &offset_nodes[0].hdr.typ.0 == b"co64";
        let chunks = if wide {
            decode::<Co64>(offset_nodes[0], data)?.entries
        } else {
            decode::<Stco>(offset_nodes[0], data)?
                .entries
                .into_iter()
                .map(u64::from)
                .collect()
        };
        let mapping = decode::<Stsc>(unique(entries, b"stsc")?, data)?.entries;
        ensure!(
            !chunks.is_empty() && !mapping.is_empty() && mapping[0].first_chunk == 1,
            "missing first chunk mapping"
        );
        let mut previous = 0;
        for entry in &mapping {
            ensure!(
                entry.first_chunk > previous
                    && entry.first_chunk as usize <= chunks.len()
                    && entry.samples_per_chunk > 0
                    && entry.sample_description_index > 0,
                "invalid sample-to-chunk mapping"
            );
            previous = entry.first_chunk;
        }
        let mut samples = Vec::new();
        let mut mapping_index = 0;
        for (chunk_index, chunk) in chunks.iter().enumerate() {
            while mapping_index + 1 < mapping.len()
                && mapping[mapping_index + 1].first_chunk as usize <= chunk_index + 1
            {
                mapping_index += 1;
            }
            let mut offset = usize::try_from(*chunk)?;
            let count = mapping[mapping_index].samples_per_chunk as usize;
            ensure!(
                count <= sizes.len() - samples.len(),
                "chunk mapping exceeds sample count"
            );
            for _ in 0..count {
                let size = sizes[samples.len()] as usize;
                let sample_end = offset.checked_add(size).context("sample range overflow")?;
                ensure!(
                    media
                        .iter()
                        .any(|r| offset >= r.start && sample_end <= r.end),
                    "sample lies outside media payload"
                );
                samples.push(offset..sample_end);
                offset = sample_end;
            }
        }
        ensure!(samples.len() == sizes.len(), "chunk mapping omits samples");
        tables.push(Table {
            path: format!("moov/trak[{index}]/mdia/minf/stbl"),
            wide,
            chunks,
            samples,
        });
    }
    ensure!(!tables.is_empty(), "no track sample tables");
    let mut ranges: Vec<_> = tables.iter().flat_map(|t| t.samples.iter()).collect();
    ranges.sort_by_key(|r| r.start);
    ensure!(
        ranges.windows(2).all(|pair| pair[0].end <= pair[1].start),
        "overlapping sample ranges"
    );
    Ok(tables)
}

fn encoded(value: &impl Encode) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    value.encode(&mut bytes)?;
    Ok(bytes)
}

fn replace(editor: &mut Editor, path: String, bytes: Vec<u8>) {
    editor.add_command(Command::Replace { path, bytes });
}

fn offset_box(table: &Table, offsets: &[u64]) -> Result<Vec<u8>> {
    if table.wide {
        encoded(&Co64 {
            entries: offsets.to_vec(),
        })
    } else {
        encoded(&Stco {
            entries: offsets
                .iter()
                .map(|n| u32::try_from(*n))
                .collect::<std::result::Result<_, _>>()
                .context("chunk offset exceeds stco capacity; co64 input required")?,
        })
    }
}

fn media_ranges(nodes: &[BoxRef], len: usize) -> Result<Vec<Range<usize>>> {
    nodes
        .iter()
        .filter(|n| &n.hdr.typ.0 == b"mdat")
        .map(|n| Ok((n.hdr.start + n.hdr.header_size) as usize..end(n, len)?))
        .collect()
}

/// Transform the first track, preserving other tracks, opaque boxes and gaps.
/// Fragmented and independently indexed byte-offset structures are rejected.
pub fn transform(data: &[u8], operation: Operation, cancel: &AtomicBool) -> Result<Converted> {
    check_cancel(cancel)?;
    let nodes = tree(data)?;
    let media = media_ranges(&nodes, data.len())?;
    ensure!(!media.is_empty(), "no mdat box");
    let tables = parse_tables(&nodes, data, &media)?;
    let selected = &tables[0];
    let mut changes = Vec::new();
    let mut sizes = Vec::new();
    let mut padding_bytes = 0;
    for (index, range) in selected.samples.iter().enumerate() {
        check_cancel(cancel)?;
        let bytes = match operation {
            Operation::Encrypt => encrypt_sample(&data[range.clone()]),
            Operation::Decrypt => decrypt_sample(&data[range.clone()])
                .with_context(|| format!("decrypting sample {index}"))?,
        };
        ensure!(!bytes.is_empty(), "decrypted audio sample is empty");
        padding_bytes += bytes.len().abs_diff(range.len());
        sizes.push(u32::try_from(bytes.len()).context("sample exceeds stsz size limit")?);
        changes.push(Change {
            range: range.clone(),
            bytes,
        });
    }
    changes.sort_by_key(|c| c.range.start);
    let mut editor = Editor::new();
    for (index, range) in media.iter().enumerate() {
        let mut payload = Vec::new();
        let mut cursor = range.start;
        for change in changes
            .iter()
            .filter(|c| c.range.start >= range.start && c.range.end <= range.end)
        {
            payload.extend_from_slice(&data[cursor..change.range.start]);
            payload.extend_from_slice(&change.bytes);
            cursor = change.range.end;
        }
        payload.extend_from_slice(&data[cursor..range.end]);
        replace(
            &mut editor,
            format!("mdat[{index}]"),
            encoded(&Mdat { data: payload })?,
        );
    }
    replace(
        &mut editor,
        format!("{}/stsz", selected.path),
        encoded(&Stsz {
            samples: StszSamples::Different { sizes },
        })?,
    );
    // Replaced offset boxes are explicitly caller-owned in mp4box's editor.
    // First lay out changed media/tables, then supply exact offsets without
    // a second size change. This avoids remapping into replaced byte extents.
    for table in &tables {
        replace(
            &mut editor,
            format!(
                "{}/{}",
                table.path,
                if table.wide { "co64" } else { "stco" }
            ),
            offset_box(table, &table.chunks)?,
        );
    }
    let mut laid_out = Vec::new();
    editor.process(&mut Cursor::new(data), &mut laid_out)?;
    let output_nodes = tree(&laid_out)?;
    let output_media = media_ranges(&output_nodes, laid_out.len())?;
    ensure!(
        output_media.len() == media.len(),
        "editor changed media box count"
    );
    let mut final_editor = Editor::new();
    for table in &tables {
        let mut offsets = Vec::new();
        for chunk in &table.chunks {
            let old = usize::try_from(*chunk)?;
            let index = media
                .iter()
                .position(|r| r.contains(&old))
                .context("chunk offset lies outside media")?;
            let mut relative = old - media[index].start;
            for change in changes
                .iter()
                .filter(|c| c.range.start >= media[index].start && c.range.end <= old)
            {
                if change.bytes.len() >= change.range.len() {
                    relative += change.bytes.len() - change.range.len();
                } else {
                    relative -= change.range.len() - change.bytes.len();
                }
            }
            offsets.push((output_media[index].start + relative) as u64);
        }
        replace(
            &mut final_editor,
            format!(
                "{}/{}",
                table.path,
                if table.wide { "co64" } else { "stco" }
            ),
            offset_box(table, &offsets)?,
        );
    }
    check_cancel(cancel)?;
    let mut bytes = Vec::new();
    final_editor.process(&mut Cursor::new(&laid_out), &mut bytes)?;
    ensure!(
        bytes.len() == laid_out.len(),
        "offset update unexpectedly changed file layout"
    );
    // Revalidate the finished sample tables, not only the intermediate layout.
    let final_nodes = tree(&bytes)?;
    let final_media = media_ranges(&final_nodes, bytes.len())?;
    let final_tables = parse_tables(&final_nodes, &bytes, &final_media)?;
    if final_tables[0].samples.len() != selected.samples.len() {
        bail!("editor changed sample count");
    }
    check_cancel(cancel)?;
    let stats = Stats {
        samples: selected.samples.len(),
        chunks: selected.chunks.len(),
        input_bytes: data.len(),
        output_bytes: bytes.len(),
        padding_bytes,
    };
    Ok(Converted { bytes, stats })
}
