use plist::Value;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

const XATTR_COMMENT: &str = "com.apple.metadata:kMDItemFinderComment";
const XATTR_TAGS: &str = "com.apple.metadata:_kMDItemUserTags";

pub fn set_finder_comment(path: &Path, comment: &str) -> std::io::Result<()> {
    let plist_val = Value::String(comment.to_string());
    let mut buf = Vec::new();
    plist_val.to_writer_binary(&mut buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    xattr::set(path, XATTR_COMMENT, &buf)?;
    Ok(())
}

fn set_finder_tags(path: &Path, tags: &[String]) -> std::io::Result<()> {
    let arr: Vec<Value> = tags.iter().map(|t| Value::String(t.clone())).collect();
    let plist_val = Value::Array(arr);
    let mut buf = Vec::new();
    plist_val.to_writer_binary(&mut buf)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    xattr::set(path, XATTR_TAGS, &buf)?;
    Ok(())
}

fn get_finder_tags(path: &Path) -> Option<Vec<String>> {
    let data = xattr::get(path, XATTR_TAGS).ok()??;
    let val = Value::from_reader(std::io::Cursor::new(&data)).ok()?;
    let arr = val.into_array()?;
    Some(arr.into_iter().filter_map(|v| v.into_string()).collect())
}

fn get_tag_value(tags: &[String], prefix: &str) -> Option<String> {
    tags.iter()
        .find(|t| t.starts_with(prefix))
        .map(|t| t[prefix.len()..].to_string())
}

fn dir_mtime(path: &Path) -> std::io::Result<u64> {
    use std::time::UNIX_EPOCH;
    let meta = fs::metadata(path)?;
    Ok(meta.modified()?.duration_since(UNIX_EPOCH).unwrap().as_secs())
}

fn count_files_in_dir(dir: &Path) -> std::io::Result<usize> {
    let mut count = 0;
    for entry in fs::read_dir(dir)? {
        if let Ok(e) = entry {
            if e.file_type().map_or(false, |ft| ft.is_file())
                && !e.file_name().to_string_lossy().starts_with('.') {
                count += 1;
            }
        }
    }
    Ok(count)
}

pub fn run_dir_count(paths: &[PathBuf], dry_run: bool) -> i32 {
    use rayon::prelude::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    let errors = AtomicI32::new(0);

    // Single-pass collection: walk each path, collect all dirs (including roots)
    let mut dirs = Vec::new();
    for path in paths {
        if !path.is_dir() {
            eprintln!("error: {}: not a directory", path.display());
            errors.fetch_add(1, Ordering::Relaxed);
            continue;
        }
        for entry in WalkDir::new(path)
            .into_iter()
            .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_dir() {
                dirs.push(entry.into_path());
            }
        }
    }

    dirs.par_iter().for_each(|dir| {
        let mtime = match dir_mtime(dir) {
            Ok(t) => t,
            Err(e) => { eprintln!("error: {}: {e}", dir.display()); errors.fetch_add(1, Ordering::Relaxed); return; }
        };

        if let Some(tags) = get_finder_tags(dir) {
            if let Some(prev) = get_tag_value(&tags, "counted:") {
                if prev == mtime.to_string() {
                    if let Some(count) = get_tag_value(&tags, "files:") {
                        println!("{count}\t{}", dir.display());
                    }
                    return;
                }
            }
        }

        let count = match count_files_in_dir(dir) {
            Ok(c) => c,
            Err(e) => { eprintln!("error: {}: {e}", dir.display()); errors.fetch_add(1, Ordering::Relaxed); return; }
        };

        if dry_run {
            println!("{count}\t{}", dir.display());
            return;
        }

        if let Err(e) = set_finder_comment(dir, &format!("{count} files")) {
            eprintln!("error: {}: {e}", dir.display()); errors.fetch_add(1, Ordering::Relaxed);
        }
        if let Err(e) = set_finder_tags(dir, &[
            format!("files:{count}"),
            format!("counted:{mtime}"),
        ]) {
            eprintln!("error: {}: {e}", dir.display()); errors.fetch_add(1, Ordering::Relaxed);
        }

        println!("{count}\t{}", dir.display());
    });

    errors.load(Ordering::Relaxed)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- math helpers ---

    #[test]
    fn next_pow2_values() {
        assert_eq!(next_pow2(1), 32);
        assert_eq!(next_pow2(32), 32);
        assert_eq!(next_pow2(33), 64);
        assert_eq!(next_pow2(64), 64);
        assert_eq!(next_pow2(100), 128);
    }

    #[test]
    fn align_up_values() {
        assert_eq!(align_up(0, 32), 0);
        assert_eq!(align_up(1, 32), 32);
        assert_eq!(align_up(32, 32), 32);
        assert_eq!(align_up(33, 32), 64);
    }

    #[test]
    fn log2_powers_of_two() {
        assert_eq!(log2(1), 0);
        assert_eq!(log2(2), 1);
        assert_eq!(log2(32), 5);
        assert_eq!(log2(1024), 10);
    }

    // --- encode_record / encode_blob ---

    #[test]
    fn encode_blob_prepends_length() {
        let data = b"hello";
        let blob = encode_blob(data);
        assert_eq!(&blob[..4], &(5u32).to_be_bytes());
        assert_eq!(&blob[4..], b"hello");
    }

    #[test]
    fn encode_record_has_filename_and_ids() {
        let rec = encode_record(".", b"icvp", b"blob", &[0xAA]);
        // filename "." is 1 UTF-16 char
        assert_eq!(&rec[..4], &(1u32).to_be_bytes());
        // UTF-16 for '.'
        assert_eq!(&rec[4..6], &(b'.' as u16).to_be_bytes());
        // structure_id + data_type
        assert_eq!(&rec[6..10], b"icvp");
        assert_eq!(&rec[10..14], b"blob");
        assert_eq!(rec[14], 0xAA);
    }

    // --- build_icvp_plist ---

    #[test]
    fn build_icvp_plist_is_valid_plist() {
        let data = build_icvp_plist(128.0, "size", true);
        let val = plist::Value::from_reader(std::io::Cursor::new(&data)).unwrap();
        let dict = val.as_dictionary().unwrap();
        assert_eq!(dict.get("iconSize").unwrap().as_real(), Some(128.0));
        assert_eq!(dict.get("arrangeBy").unwrap().as_string(), Some("size"));
        assert_eq!(dict.get("showIconPreview").unwrap().as_boolean(), Some(true));
    }

    // --- build_ds_store ---

    #[test]
    fn build_ds_store_starts_with_magic() {
        let data = build_ds_store(128, "size", true).unwrap();
        // DS_Store starts with 0x00000001 then "Bud1"
        assert_eq!(&data[..4], &[0, 0, 0, 1]);
        assert_eq!(&data[4..8], b"Bud1");
    }

    #[test]
    fn build_ds_store_min_size() {
        let data = build_ds_store(64, "name", false).unwrap();
        // Should be a reasonable size (header + master + leaf + bookkeeping)
        assert!(data.len() > 100);
    }

    // --- compile_excludes / is_excluded ---

    #[test]
    fn compile_excludes_strips_trailing_slash() {
        let excl = compile_excludes(&["node_modules/".to_string()]);
        assert_eq!(excl.len(), 1);
        assert!(is_excluded(Path::new("project/node_modules/pkg"), &excl));
    }

    #[test]
    fn is_excluded_matches_filename() {
        let excl = compile_excludes(&[".DS_Store".to_string()]);
        assert!(is_excluded(Path::new("dir/.DS_Store"), &excl));
        assert!(!is_excluded(Path::new("dir/file.txt"), &excl));
    }

    // --- resolve_paths ---

    #[test]
    fn resolve_paths_finds_files() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::write(dir.path().join("b.txt"), "b").unwrap();
        let files = resolve_paths(&[dir.path().to_string_lossy().to_string()], true, &[]);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn resolve_paths_recursive_finds_nested() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(dir.path().join("top.txt"), "t").unwrap();
        fs::write(sub.join("deep.txt"), "d").unwrap();
        let files = resolve_paths(&[dir.path().to_string_lossy().to_string()], true, &[]);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn resolve_paths_nonrecursive() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(dir.path().join("top.txt"), "t").unwrap();
        fs::write(sub.join("deep.txt"), "d").unwrap();
        let files = resolve_paths(&[dir.path().to_string_lossy().to_string()], false, &[]);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn resolve_paths_excludes() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("keep.txt"), "k").unwrap();
        fs::write(dir.path().join("skip.log"), "s").unwrap();
        let excl = compile_excludes(&["*.log".to_string()]);
        let files = resolve_paths(&[dir.path().to_string_lossy().to_string()], true, &excl);
        assert_eq!(files.len(), 1);
    }

    // --- run_write / run_comment integration ---

    #[test]
    fn run_write_creates_ds_store() {
        let dir = TempDir::new().unwrap();
        let code = run_write(dir.path(), 128, "size", true, false, false);
        assert_eq!(code, 0);
        assert!(dir.path().join(".DS_Store").exists());
    }

    #[test]
    fn run_write_dry_run_does_not_create_file() {
        let dir = TempDir::new().unwrap();
        let code = run_write(dir.path(), 128, "size", true, true, false);
        assert_eq!(code, 0);
        assert!(!dir.path().join(".DS_Store").exists());
    }

    #[test]
    fn run_write_clean_removes_ds_store() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".DS_Store"), "dummy").unwrap();
        let code = run_write(dir.path(), 128, "size", true, false, true);
        assert_eq!(code, 0);
        assert!(!dir.path().join(".DS_Store").exists());
    }

    #[test]
    fn run_comment_dry_run() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("file.txt"), "content").unwrap();
        let code = run_comment("test comment", &[dir.path().to_string_lossy().to_string()], true, &[], true);
        assert_eq!(code, 0);
    }

    #[test]
    fn run_comment_sets_xattr() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        fs::write(&file, "content").unwrap();
        let code = run_comment("my comment", &[file.to_string_lossy().to_string()], false, &[], false);
        assert_eq!(code, 0);
        let data = xattr::get(&file, XATTR_COMMENT).unwrap().unwrap();
        let val = plist::Value::from_reader(std::io::Cursor::new(&data)).unwrap();
        assert_eq!(val.as_string(), Some("my comment"));
    }
}
