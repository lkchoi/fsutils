use plist::Value;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const XATTR_COMMENT: &str = "com.apple.metadata:kMDItemFinderComment";

pub fn set_finder_comment(path: &Path, comment: &str) -> std::io::Result<()> {
    let plist_val = Value::String(comment.to_string());
    let mut buf = Vec::new();
    plist_val.to_writer_binary(&mut buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    xattr::set(path, XATTR_COMMENT, &buf)?;
    Ok(())
}

pub fn is_excluded(path: &Path, exclude: &[glob::Pattern]) -> bool {
    let path_str = path.to_string_lossy();
    for pat in exclude {
        if pat.matches(&path_str) { return true; }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if pat.matches(name) { return true; }
        }
        for component in path.components() {
            if let Some(s) = component.as_os_str().to_str() {
                if pat.matches(s) { return true; }
            }
        }
    }
    false
}

pub fn compile_excludes(patterns: &[String]) -> Vec<glob::Pattern> {
    patterns.iter().filter_map(|p| {
        glob::Pattern::new(p.trim_end_matches('/'))
            .map_err(|e| eprintln!("Invalid exclude pattern '{p}': {e}"))
            .ok()
    }).collect()
}

fn collect_dir_files(dir: &Path, exclude: &[glob::Pattern], files: &mut Vec<PathBuf>) {
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), exclude))
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() { files.push(entry.into_path()); }
    }
}

pub fn resolve_paths(patterns: &[String], recursive: bool, exclude: &[glob::Pattern]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for pattern in patterns {
        let path = Path::new(pattern);
        if pattern.contains('*') || pattern.contains('?') || pattern.contains('[') {
            if let Ok(entries) = glob::glob(pattern) {
                for entry in entries.flatten() {
                    if is_excluded(&entry, exclude) { continue; }
                    if entry.is_file() { files.push(entry); }
                    else if entry.is_dir() && recursive { collect_dir_files(&entry, exclude, &mut files); }
                }
            }
        } else if path.is_file() {
            if !is_excluded(path, exclude) { files.push(path.to_path_buf()); }
        } else if path.is_dir() && recursive {
            collect_dir_files(path, exclude, &mut files);
        } else if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    let p = entry.path();
                    if !is_excluded(&p, exclude) && p.is_file() { files.push(p); }
                }
            }
        } else {
            eprintln!("Path not found: {pattern}");
        }
    }
    files
}

pub fn run_comment(comment: &str, paths: &[String], recursive: bool, exclude: &[String], dry_run: bool) -> i32 {
    let exclude = compile_excludes(exclude);
    let files = resolve_paths(paths, recursive, &exclude);
    if files.is_empty() { eprintln!("No files found."); return 1; }

    let mut errors = 0i32;
    for file in &files {
        if dry_run { println!("{}", file.display()); continue; }
        if let Err(e) = set_finder_comment(file, comment) {
            eprintln!("error: {}: {e}", file.display()); errors += 1;
        }
    }
    errors
}

pub fn run_write(path: &Path, icon_size: u16, arrange_by: &str, show_preview: bool, dry_run: bool, clean: bool) -> i32 {
    let mut count = 0u64;
    let mut errors = 0i32;

    for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_dir() { continue; }
        let ds_store_path = entry.path().join(".DS_Store");

        if clean {
            if ds_store_path.exists() {
                if dry_run { println!("rm {}", ds_store_path.display()); count += 1; }
                else {
                    match fs::remove_file(&ds_store_path) {
                        Ok(_) => count += 1,
                        Err(e) => { eprintln!("error: {}: {}", ds_store_path.display(), e); errors += 1; }
                    }
                }
            }
            continue;
        }

        if dry_run { println!("{}", ds_store_path.display()); count += 1; continue; }

        match write_ds_store(&ds_store_path, icon_size, arrange_by, show_preview) {
            Ok(_) => count += 1,
            Err(e) => { eprintln!("error: {}: {}", ds_store_path.display(), e); errors += 1; }
        }
    }

    if clean { println!("{count} deleted, {errors} errors"); }
    else if dry_run { println!("\n{count} directories (dry run)"); }
    else { println!("{count} written, {errors} errors"); }
    errors
}

fn write_ds_store(path: &Path, icon_size: u16, arrange_by: &str, show_preview: bool) -> std::io::Result<()> {
    let data = build_ds_store(icon_size, arrange_by, show_preview)?;
    fs::write(path, data)
}

fn build_ds_store(icon_size: u16, arrange_by: &str, show_preview: bool) -> std::io::Result<Vec<u8>> {
    let plist_data = build_icvp_plist(icon_size as f64, arrange_by, show_preview);
    let mut records: Vec<Vec<u8>> = vec![
        encode_record(".", b"icvp", b"blob", &encode_blob(&plist_data)),
        encode_record(".", b"vSrn", b"long", &1u32.to_be_bytes()),
    ];
    records.sort();

    let mut leaf = Vec::new();
    leaf.extend_from_slice(&0u32.to_be_bytes());
    leaf.extend_from_slice(&(records.len() as u32).to_be_bytes());
    for rec in &records { leaf.extend_from_slice(rec); }
    let leaf_block_size = next_pow2(leaf.len().max(32));
    leaf.resize(leaf_block_size, 0);

    let mut master = Vec::with_capacity(32);
    master.extend_from_slice(&1u32.to_be_bytes());
    master.extend_from_slice(&0u32.to_be_bytes());
    master.extend_from_slice(&(records.len() as u32).to_be_bytes());
    master.extend_from_slice(&1u32.to_be_bytes());
    master.extend_from_slice(&0x1000u32.to_be_bytes());
    master.resize(32, 0);

    let master_buddy_off: u32 = 32;
    let leaf_buddy_off = align_up(master_buddy_off + 32, leaf_block_size as u32);
    let bk_buddy_off = leaf_buddy_off + leaf_block_size as u32;

    let block0_addr = master_buddy_off | log2(32);
    let block1_addr = leaf_buddy_off | log2(leaf_block_size as u32);

    let bookkeeping = build_bookkeeping(block0_addr, block1_addr);
    let bk_size = bookkeeping.len() as u32;

    let file_size = (4 + bk_buddy_off + bk_size) as usize;
    let mut out = Vec::with_capacity(file_size);

    out.extend_from_slice(&0x0000_0001u32.to_be_bytes());
    out.extend_from_slice(b"Bud1");
    out.extend_from_slice(&bk_buddy_off.to_be_bytes());
    out.extend_from_slice(&bk_size.to_be_bytes());
    out.extend_from_slice(&bk_buddy_off.to_be_bytes());
    out.extend_from_slice(&[0u8; 16]);

    debug_assert_eq!(out.len(), 4 + master_buddy_off as usize);
    out.extend_from_slice(&master);
    out.resize(4 + leaf_buddy_off as usize, 0);

    debug_assert_eq!(out.len(), 4 + leaf_buddy_off as usize);
    out.extend_from_slice(&leaf);

    debug_assert_eq!(out.len(), 4 + bk_buddy_off as usize);
    out.extend_from_slice(&bookkeeping);

    Ok(out)
}

fn build_bookkeeping(block0_addr: u32, block1_addr: u32) -> Vec<u8> {
    let mut bk = Vec::new();
    bk.extend_from_slice(&2u32.to_be_bytes());
    bk.extend_from_slice(&0u32.to_be_bytes());
    bk.extend_from_slice(&block0_addr.to_be_bytes());
    bk.extend_from_slice(&block1_addr.to_be_bytes());
    for _ in 2..256 { bk.extend_from_slice(&0u32.to_be_bytes()); }
    bk.extend_from_slice(&1u32.to_be_bytes());
    bk.push(4);
    bk.extend_from_slice(b"DSDB");
    bk.extend_from_slice(&0u32.to_be_bytes());
    for _ in 0..32 { bk.extend_from_slice(&0u32.to_be_bytes()); }
    bk
}

fn build_icvp_plist(icon_size: f64, arrange_by: &str, show_preview: bool) -> Vec<u8> {
    let mut dict = plist::Dictionary::new();
    dict.insert("viewOptionsVersion".into(), Value::Integer(1.into()));
    dict.insert("arrangeBy".into(), Value::String(arrange_by.into()));
    dict.insert("showIconPreview".into(), Value::Boolean(show_preview));
    dict.insert("showItemInfo".into(), Value::Boolean(false));
    dict.insert("labelOnBottom".into(), Value::Boolean(true));
    dict.insert("iconSize".into(), Value::Real(icon_size));
    dict.insert("textSize".into(), Value::Real(12.0));
    dict.insert("gridSpacing".into(), Value::Real(54.0));
    dict.insert("gridOffsetX".into(), Value::Real(0.0));
    dict.insert("gridOffsetY".into(), Value::Real(0.0));
    let mut buf = Vec::new();
    Value::Dictionary(dict).to_writer_binary(&mut buf).expect("plist serialization failed");
    buf
}

fn encode_record(filename: &str, structure_id: &[u8; 4], data_type: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut rec = Vec::new();
    let chars: Vec<u16> = filename.encode_utf16().collect();
    rec.extend_from_slice(&(chars.len() as u32).to_be_bytes());
    for ch in &chars { rec.extend_from_slice(&ch.to_be_bytes()); }
    rec.extend_from_slice(structure_id);
    rec.extend_from_slice(data_type);
    rec.extend_from_slice(data);
    rec
}

fn encode_blob(data: &[u8]) -> Vec<u8> {
    let mut blob = Vec::with_capacity(4 + data.len());
    blob.extend_from_slice(&(data.len() as u32).to_be_bytes());
    blob.extend_from_slice(data);
    blob
}

fn next_pow2(n: usize) -> usize {
    let mut p = 32usize;
    while p < n { p <<= 1; }
    p
}

fn align_up(offset: u32, alignment: u32) -> u32 {
    (offset + alignment - 1) & !(alignment - 1)
}

fn log2(n: u32) -> u32 {
    debug_assert!(n.is_power_of_two());
    n.trailing_zeros()
}
