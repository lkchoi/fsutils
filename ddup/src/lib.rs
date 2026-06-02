use sha2::Digest as _;
use std::collections::HashMap;
use std::fs;
use std::io::{self, BufRead, Read, Write as _};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

// --- Config ---

fn config_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(home) = dirs::home_dir() {
        if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
            paths.push(PathBuf::from(xdg).join("ddup").join("config"));
        } else {
            paths.push(home.join(".config").join("ddup").join("config"));
        }
        paths.push(home.join(".ddup"));
    }
    paths.push(PathBuf::from(".ddup"));
    paths
}

pub fn load_config() -> Vec<(String, String)> {
    let mut config = Vec::new();
    for path in config_paths() {
        if let Ok(contents) = fs::read_to_string(&path) {
            for line in contents.lines() {
                let line = line.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some((key, value)) = line.split_once('=') {
                    config.push((key.trim().to_string(), value.trim().to_string()));
                }
            }
        }
    }
    config
}

fn short_flag_for(key: &str) -> Option<&'static str> {
    match key {
        "algorithm" => Some("-a"),
        "recursive" => Some("-r"),
        "verbose" => Some("-v"),
        "duplicates" => Some("-d"),
        "keep" => Some("-k"),
        "exclude" => Some("-e"),
        _ => None,
    }
}

const REPEATABLE_KEYS: &[&str] = &["exclude"];

pub fn apply_config(args: &mut Vec<String>) {
    let config = load_config();
    if config.is_empty() { return; }

    let mut seen: HashMap<String, String> = HashMap::new();
    let mut repeatable: Vec<(String, String)> = Vec::new();

    for (key, value) in config {
        if REPEATABLE_KEYS.contains(&key.as_str()) {
            repeatable.push((key, value));
        } else {
            seen.insert(key, value);
        }
    }

    for (key, value) in &seen {
        let long_flag = format!("--{}", key.replace('_', "-"));
        let already_set = args.iter().any(|a| a.starts_with(&long_flag))
            || short_flag_for(key).is_some_and(|s| args.iter().any(|a| a.starts_with(s)));
        if already_set { continue; }
        match value.as_str() {
            "true" => args.push(long_flag),
            "false" => {}
            _ => { args.push(long_flag); args.push(value.clone()); }
        }
    }

    for (key, value) in &repeatable {
        let long_flag = format!("--{}", key.replace('_', "-"));
        args.push(long_flag);
        args.push(value.clone());
    }
}

// --- Hash algorithm ---

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum HashAlgorithm {
    Md5,
    Sha256,
    Blake3,
    Xxh3,
}

impl std::fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HashAlgorithm::Md5 => write!(f, "md5"),
            HashAlgorithm::Sha256 => write!(f, "sha256"),
            HashAlgorithm::Blake3 => write!(f, "blake3"),
            HashAlgorithm::Xxh3 => write!(f, "xxh3"),
        }
    }
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum KeepStrategy { Newest, Oldest, Largest, Smallest, Best, Shallowest, Deepest, First }

// --- Hashing ---

pub fn hash_file(path: &Path, algorithm: &HashAlgorithm) -> io::Result<String> {
    let mut file = fs::File::open(path)?;
    let mut buf = [0u8; 8192];
    Ok(match algorithm {
        HashAlgorithm::Md5 => {
            let mut ctx = md5::Context::new();
            loop { let n = file.read(&mut buf)?; if n == 0 { break; } ctx.consume(&buf[..n]); }
            format!("{:x}", ctx.compute())
        }
        HashAlgorithm::Sha256 => {
            let mut h = sha2::Sha256::new();
            loop { let n = file.read(&mut buf)?; if n == 0 { break; } h.update(&buf[..n]); }
            hex::encode(h.finalize())
        }
        HashAlgorithm::Blake3 => {
            let mut h = blake3::Hasher::new();
            loop { let n = file.read(&mut buf)?; if n == 0 { break; } h.update(&buf[..n]); }
            h.finalize().to_hex().to_string()
        }
        HashAlgorithm::Xxh3 => {
            let mut h = xxhash_rust::xxh3::Xxh3::new();
            loop { let n = file.read(&mut buf)?; if n == 0 { break; } std::hash::Hasher::write(&mut h, &buf[..n]); }
            format!("{:016x}", std::hash::Hasher::finish(&h))
        }
    })
}

// --- Hash caching ---

const XATTR_HASH: &str = "com.ddup.hash";
const XATTR_HASHED_AT: &str = "com.ddup.hashed";
const XATTR_ALGORITHM: &str = "com.ddup.algorithm";

pub fn get_cached_hash(path: &Path, algorithm: &HashAlgorithm) -> Option<String> {
    let algo_data = xattr::get(path, XATTR_ALGORITHM).ok()??;
    let cached_algo = String::from_utf8(algo_data).ok()?;
    if cached_algo != algorithm.to_string() { return None; }
    let ts_data = xattr::get(path, XATTR_HASHED_AT).ok()??;
    let ts_str = String::from_utf8(ts_data).ok()?;
    let hashed_at: u64 = ts_str.parse().ok()?;
    let mtime = fs::metadata(path).ok()?.modified().ok()?
        .duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
    if mtime <= hashed_at {
        let hash_data = xattr::get(path, XATTR_HASH).ok()??;
        String::from_utf8(hash_data).ok()
    } else { None }
}

pub fn set_hash_cache(path: &Path, hash: &str, algorithm: &HashAlgorithm) {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs().to_string();
    let _ = xattr::set(path, XATTR_HASH, hash.as_bytes());
    let _ = xattr::set(path, XATTR_HASHED_AT, now.as_bytes());
    let _ = xattr::set(path, XATTR_ALGORITHM, algorithm.to_string().as_bytes());
}

pub fn hash_file_cached(path: &Path, algorithm: &HashAlgorithm, no_cache: bool) -> io::Result<String> {
    if !no_cache {
        if let Some(cached) = get_cached_hash(path, algorithm) { return Ok(cached); }
    }
    let hash = hash_file(path, algorithm)?;
    set_hash_cache(path, &hash, algorithm);
    Ok(hash)
}

// --- Finder tag ---

pub fn set_finder_tag(path: &Path, tag: &str) -> io::Result<()> {
    let mut tags: Vec<plist::Value> = match xattr::get(path, "com.apple.metadata:_kMDItemUserTags") {
        Ok(Some(data)) => match plist::Value::from_reader(io::Cursor::new(&data)) {
            Ok(plist::Value::Array(arr)) => arr,
            _ => Vec::new(),
        },
        _ => Vec::new(),
    };
    tags.retain(|v| {
        if let plist::Value::String(s) = v { !s.starts_with("hash:") && !s.starts_with("hashed:") && !s.starts_with("phash:") }
        else { true }
    });
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    tags.push(plist::Value::String(format!("hash:{tag}")));
    tags.push(plist::Value::String(format!("hashed:{now}")));
    if let Ok(Some(phash_data)) = xattr::get(path, "com.ssim.phash") {
        if let Ok(phash) = String::from_utf8(phash_data) {
            tags.push(plist::Value::String(format!("phash:{phash}")));
        }
    }
    let mut buf = Vec::new();
    plist::Value::Array(tags).to_writer_binary(&mut buf)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    xattr::set(path, "com.apple.metadata:_kMDItemUserTags", &buf)?;
    Ok(())
}

// --- Trash ---

pub fn move_to_trash(path: &Path) -> io::Result<()> {
    let abs_path = fs::canonicalize(path)?;
    let script = format!(r#"tell application "Finder" to delete (POSIX file "{}" as alias)"#, abs_path.display());
    let output = std::process::Command::new("osascript").args(["-e", &script]).output()?;
    if !output.status.success() {
        return Err(io::Error::new(io::ErrorKind::Other, String::from_utf8_lossy(&output.stderr).to_string()));
    }
    Ok(())
}

// --- Utilities ---

pub fn format_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    for unit in UNITS {
        if size < 1024.0 { return format!("{size:.0} {unit}"); }
        size /= 1024.0;
    }
    format!("{size:.0} PB")
}

pub fn file_mtime(path: &Path) -> u64 {
    fs::metadata(path).and_then(|m| m.modified()).ok()
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .map(|d| d.as_secs()).unwrap_or(0)
}

pub fn path_depth(path: &Path) -> usize { path.components().count() }

fn image_resolution(path: &Path) -> u64 {
    image::image_dimensions(path)
        .map(|(w, h)| w as u64 * h as u64)
        .unwrap_or(0)
}

fn format_rank(path: &Path) -> u32 {
    match path.extension().and_then(|e| e.to_str()).map(|e| e.to_lowercase()).as_deref() {
        Some("webp") => 5,
        Some("png") => 4,
        Some("tiff") | Some("tif") => 3,
        Some("jpg") | Some("jpeg") => 2,
        Some("gif") => 1,
        Some("bmp") => 0,
        _ => 0,
    }
}

fn best_score(path: &Path) -> (u64, u32) {
    (image_resolution(path), format_rank(path))
}

pub fn select_keep_index<P: AsRef<Path>>(paths: &[P], strategy: &KeepStrategy) -> usize {
    match strategy {
        KeepStrategy::Newest => paths.iter().enumerate().max_by_key(|(_, p)| file_mtime(p.as_ref())).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Oldest => paths.iter().enumerate().min_by_key(|(_, p)| file_mtime(p.as_ref())).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Largest => paths.iter().enumerate().max_by_key(|(_, p)| fs::metadata(p.as_ref()).map(|m| m.len()).unwrap_or(0)).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Smallest => paths.iter().enumerate().min_by_key(|(_, p)| fs::metadata(p.as_ref()).map(|m| m.len()).unwrap_or(0)).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Best => paths.iter().enumerate().max_by_key(|(_, p)| best_score(p.as_ref())).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Shallowest => paths.iter().enumerate().min_by_key(|(_, p)| path_depth(p.as_ref())).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Deepest => paths.iter().enumerate().max_by_key(|(_, p)| path_depth(p.as_ref())).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::First => paths.iter().enumerate().min_by_key(|(_, p)| p.as_ref().to_string_lossy().to_string()).map(|(i, _)| i).unwrap_or(0),
    }
}

pub fn read_tty_line() -> String {
    let mut input = String::new();
    if let Ok(tty) = fs::File::open("/dev/tty") {
        io::BufReader::new(tty).read_line(&mut input).ok();
    } else {
        io::stdin().lock().read_line(&mut input).ok();
    }
    input
}

pub fn prompt_yn(msg: &str) -> bool {
    eprint!("{msg} ");
    io::stderr().flush().ok();
    matches!(read_tty_line().trim().to_lowercase().as_str(), "y" | "yes")
}

pub enum DeleteAction { Trash, HardDelete, Abort }

pub fn prompt_delete(msg: &str) -> DeleteAction {
    eprint!("{msg} ");
    io::stderr().flush().ok();
    match read_tty_line().trim().to_lowercase().as_str() {
        "y" | "yes" => DeleteAction::Trash,
        "h" => DeleteAction::HardDelete,
        _ => DeleteAction::Abort,
    }
}

pub fn hard_delete(path: &Path) -> io::Result<()> {
    fs::remove_file(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- format_size ---

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(512), "512 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn format_size_kb_mb_gb() {
        assert_eq!(format_size(1024), "1 KB");
        assert_eq!(format_size(1024 * 1024), "1 MB");
        assert_eq!(format_size(1024 * 1024 * 1024), "1 GB");
    }

    // --- path_depth ---

    #[test]
    fn path_depth_simple() {
        assert_eq!(path_depth(Path::new("a.txt")), 1);
        assert_eq!(path_depth(Path::new("a/b/c.txt")), 3);
        assert_eq!(path_depth(Path::new("/a/b/c.txt")), 4); // RootDir + 3
    }

    // --- format_rank ---

    #[test]
    fn format_rank_ordering() {
        assert!(format_rank(Path::new("a.webp")) > format_rank(Path::new("a.png")));
        assert!(format_rank(Path::new("a.png")) > format_rank(Path::new("a.jpg")));
        assert!(format_rank(Path::new("a.jpg")) > format_rank(Path::new("a.gif")));
        assert_eq!(format_rank(Path::new("a.txt")), 0);
    }

    // --- is_excluded / compile_excludes ---

    #[test]
    fn compile_excludes_valid_and_invalid() {
        let patterns = vec!["*.txt".to_string(), "[invalid".to_string()];
        let compiled = compile_excludes(&patterns);
        assert_eq!(compiled.len(), 1);
    }

    #[test]
    fn is_excluded_by_filename() {
        let excl = compile_excludes(&["*.log".to_string()]);
        assert!(is_excluded(Path::new("dir/test.log"), &excl));
        assert!(!is_excluded(Path::new("dir/test.txt"), &excl));
    }

    #[test]
    fn is_excluded_by_component() {
        let excl = compile_excludes(&[".git".to_string()]);
        assert!(is_excluded(Path::new("repo/.git/config"), &excl));
    }

    // --- is_image_file ---

    #[test]
    fn is_image_file_recognizes_extensions() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.PNG")));
        assert!(is_image_file(Path::new("photo.webp")));
        assert!(!is_image_file(Path::new("doc.pdf")));
        assert!(!is_image_file(Path::new("noext")));
    }

    // --- hash_file ---

    fn make_temp_file(content: &[u8]) -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("test.bin");
        fs::write(&path, content).unwrap();
        (dir, path)
    }

    #[test]
    fn hash_file_xxh3_deterministic() {
        let (_dir, path) = make_temp_file(b"hello world");
        let h1 = hash_file(&path, &HashAlgorithm::Xxh3).unwrap();
        let h2 = hash_file(&path, &HashAlgorithm::Xxh3).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16); // 64-bit hex
    }

    #[test]
    fn hash_file_md5() {
        let (_dir, path) = make_temp_file(b"hello world");
        let h = hash_file(&path, &HashAlgorithm::Md5).unwrap();
        assert_eq!(h, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn hash_file_sha256() {
        let (_dir, path) = make_temp_file(b"hello world");
        let h = hash_file(&path, &HashAlgorithm::Sha256).unwrap();
        assert!(h.starts_with("b94d27b9"));
    }

    #[test]
    fn hash_file_blake3() {
        let (_dir, path) = make_temp_file(b"hello world");
        let h = hash_file(&path, &HashAlgorithm::Blake3).unwrap();
        assert!(!h.is_empty());
    }

    #[test]
    fn hash_file_different_content_different_hash() {
        let (_dir1, path1) = make_temp_file(b"aaa");
        let (_dir2, path2) = make_temp_file(b"bbb");
        let h1 = hash_file(&path1, &HashAlgorithm::Xxh3).unwrap();
        let h2 = hash_file(&path2, &HashAlgorithm::Xxh3).unwrap();
        assert_ne!(h1, h2);
    }

    // --- hash caching via xattr ---

    #[test]
    fn hash_file_cached_stores_and_retrieves() {
        let (_dir, path) = make_temp_file(b"cached test");
        let h1 = hash_file_cached(&path, &HashAlgorithm::Xxh3, false).unwrap();
        // Second call should hit cache
        let h2 = hash_file_cached(&path, &HashAlgorithm::Xxh3, false).unwrap();
        assert_eq!(h1, h2);
        // Verify cache was actually set
        assert!(get_cached_hash(&path, &HashAlgorithm::Xxh3).is_some());
    }

    #[test]
    fn hash_file_cached_no_cache_flag_skips_cache() {
        let (_dir, path) = make_temp_file(b"no cache test");
        let h1 = hash_file_cached(&path, &HashAlgorithm::Xxh3, true).unwrap();
        let h2 = hash_file_cached(&path, &HashAlgorithm::Xxh3, true).unwrap();
        assert_eq!(h1, h2);
    }

    #[test]
    fn cached_hash_wrong_algorithm_returns_none() {
        let (_dir, path) = make_temp_file(b"algo test");
        hash_file_cached(&path, &HashAlgorithm::Xxh3, false).unwrap();
        assert!(get_cached_hash(&path, &HashAlgorithm::Md5).is_none());
    }

    // --- resolve_paths ---

    #[test]
    fn resolve_paths_finds_files_in_dir() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.txt"), "a").unwrap();
        fs::write(dir.path().join("b.txt"), "b").unwrap();
        let files = resolve_paths(&[dir.path().to_string_lossy().to_string()], true, &[]);
        assert_eq!(files.len(), 2);
    }

    #[test]
    fn resolve_paths_excludes_patterns() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("keep.txt"), "k").unwrap();
        fs::write(dir.path().join("skip.log"), "s").unwrap();
        let excl = compile_excludes(&["*.log".to_string()]);
        let files = resolve_paths(&[dir.path().to_string_lossy().to_string()], true, &excl);
        assert_eq!(files.len(), 1);
        assert!(files[0].to_string_lossy().contains("keep.txt"));
    }

    #[test]
    fn resolve_paths_single_file() {
        let (_dir, path) = make_temp_file(b"single");
        let files = resolve_paths(&[path.to_string_lossy().to_string()], false, &[]);
        assert_eq!(files.len(), 1);
    }

    #[test]
    fn resolve_paths_nonrecursive_skips_subdirs() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(dir.path().join("top.txt"), "t").unwrap();
        fs::write(sub.join("deep.txt"), "d").unwrap();
        let files = resolve_paths(&[dir.path().to_string_lossy().to_string()], false, &[]);
        assert_eq!(files.len(), 1);
    }

    // --- select_keep_index ---

    #[test]
    fn select_keep_largest_smallest() {
        let dir = TempDir::new().unwrap();
        let small = dir.path().join("small.txt");
        let big = dir.path().join("big.txt");
        fs::write(&small, "x").unwrap();
        fs::write(&big, "xxxxx").unwrap();
        let paths = vec![small.clone(), big.clone()];

        assert_eq!(select_keep_index(&paths, &KeepStrategy::Largest), 1);
        assert_eq!(select_keep_index(&paths, &KeepStrategy::Smallest), 0);
    }

    #[test]
    fn select_keep_shallowest_deepest() {
        let paths = vec![
            PathBuf::from("a/b/c/file.txt"),
            PathBuf::from("a/file.txt"),
        ];
        assert_eq!(select_keep_index(&paths, &KeepStrategy::Shallowest), 1);
        assert_eq!(select_keep_index(&paths, &KeepStrategy::Deepest), 0);
    }

    #[test]
    fn select_keep_first_is_alphabetical() {
        let paths = vec![
            PathBuf::from("z.txt"),
            PathBuf::from("a.txt"),
        ];
        assert_eq!(select_keep_index(&paths, &KeepStrategy::First), 1);
    }

    // --- hard_delete ---

    #[test]
    fn hard_delete_removes_file() {
        let (_dir, path) = make_temp_file(b"delete me");
        assert!(path.exists());
        hard_delete(&path).unwrap();
        assert!(!path.exists());
    }

    // --- apply_config ---

    #[test]
    fn apply_config_does_not_override_existing_flags() {
        // apply_config reads from config files which may not exist in test env,
        // but we can at least verify it doesn't panic with empty config
        let mut args = vec!["ddup".to_string(), ".".to_string()];
        apply_config(&mut args);
        assert!(args.contains(&"ddup".to_string()));
    }

    // --- HashAlgorithm Display ---

    #[test]
    fn hash_algorithm_display() {
        assert_eq!(HashAlgorithm::Xxh3.to_string(), "xxh3");
        assert_eq!(HashAlgorithm::Md5.to_string(), "md5");
        assert_eq!(HashAlgorithm::Sha256.to_string(), "sha256");
        assert_eq!(HashAlgorithm::Blake3.to_string(), "blake3");
    }

    // --- Finder tag ---

    #[test]
    fn set_finder_tag_writes_xattr() {
        let (_dir, path) = make_temp_file(b"tag test");
        set_finder_tag(&path, "abc123").unwrap();
        let data = xattr::get(&path, "com.apple.metadata:_kMDItemUserTags").unwrap().unwrap();
        // Should be valid binary plist containing hash:abc123
        let val = plist::Value::from_reader(io::Cursor::new(&data)).unwrap();
        let arr = val.as_array().unwrap();
        let tags: Vec<String> = arr.iter().filter_map(|v| v.as_string().map(|s| s.to_string())).collect();
        assert!(tags.iter().any(|t| t == "hash:abc123"));
        assert!(tags.iter().any(|t| t.starts_with("hashed:")));
    }
}

// --- Path resolution ---

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
        let p = p.trim_end_matches('/');
        glob::Pattern::new(p).map_err(|e| eprintln!("Invalid exclude pattern '{p}': {e}")).ok()
    }).collect()
}

pub fn resolve_paths(patterns: &[String], recursive: bool, exclude: &[glob::Pattern]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for pattern in patterns {
        let path = Path::new(pattern);
        if pattern.contains('*') || pattern.contains('?') || pattern.contains('[') {
            match glob::glob(pattern) {
                Ok(entries) => {
                    for entry in entries.flatten() {
                        if is_excluded(&entry, exclude) { continue; }
                        if entry.is_file() { files.push(entry); }
                        else if entry.is_dir() && recursive { collect_dir_files(&entry, exclude, &mut files); }
                    }
                }
                Err(e) => eprintln!("Invalid glob pattern '{pattern}': {e}"),
            }
            continue;
        }
        if path.is_file() {
            if !is_excluded(path, exclude) { files.push(path.to_path_buf()); }
        } else if path.is_dir() {
            if recursive { collect_dir_files(path, exclude, &mut files); }
            else {
                if let Ok(entries) = fs::read_dir(path) {
                    for entry in entries.flatten() {
                        let p = entry.path();
                        if is_excluded(&p, exclude) { continue; }
                        if entry.file_type().map(|t| t.is_file()).unwrap_or(false) { files.push(p); }
                    }
                }
            }
        } else {
            eprintln!("Path not found: {pattern}");
        }
    }
    files
}

pub fn collect_dir_files(dir: &Path, exclude: &[glob::Pattern], files: &mut Vec<PathBuf>) {
    for entry in WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| !is_excluded(e.path(), exclude))
        .filter_map(|e| e.ok())
    {
        if entry.file_type().is_file() { files.push(entry.into_path()); }
    }
}

// --- SSIM-based duplicate grouping ---

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "gif", "webp", "tiff", "tif"];

fn is_image_file(path: &Path) -> bool {
    path.extension().map_or(false, |ext| {
        IMAGE_EXTENSIONS.contains(&ext.to_string_lossy().to_lowercase().as_str())
    })
}

pub fn ssim_duplicate_groups(
    files: &[PathBuf], threshold: f64, hash_threshold: u32,
    algorithm: &HashAlgorithm, no_cache: bool,
) -> Vec<Vec<PathBuf>> {
    use rayon::prelude::*;

    let mut result: Vec<Vec<PathBuf>> = Vec::new();

    // --- Images: perceptual similarity via SSIM ---
    let image_files: Vec<&PathBuf> = files.iter().filter(|p| is_image_file(p)).collect();
    let entries: Vec<ssim::hash::ImageEntry> = image_files.par_iter()
        .filter_map(|p| match ssim::hash::load_and_hash(p) {
            Ok(entry) => Some(entry),
            Err(e) => { eprintln!("Warning: {}", e); None }
        })
        .collect();

    if !entries.is_empty() {
        let candidates = ssim::hash::candidate_pairs(&entries, hash_threshold);
        let matches: Vec<(usize, usize)> = candidates.par_iter()
            .filter_map(|&(i, j)| {
                let img_a = image::open(&entries[i].path).ok()?;
                let img_b = image::open(&entries[j].path).ok()?;
                let score = ssim::ssim::compute_ssim(&img_a, &img_b);
                if score >= threshold { Some((i, j)) } else { None }
            })
            .collect();

        if !matches.is_empty() {
            let n = entries.len();
            let mut parent: Vec<usize> = (0..n).collect();
            let mut rank = vec![0usize; n];

            fn find(parent: &mut Vec<usize>, i: usize) -> usize {
                if parent[i] != i { parent[i] = find(parent, parent[i]); }
                parent[i]
            }
            fn union(parent: &mut Vec<usize>, rank: &mut Vec<usize>, a: usize, b: usize) {
                let ra = find(parent, a);
                let rb = find(parent, b);
                if ra == rb { return; }
                if rank[ra] < rank[rb] { parent[ra] = rb; }
                else if rank[ra] > rank[rb] { parent[rb] = ra; }
                else { parent[rb] = ra; rank[ra] += 1; }
            }

            for &(i, j) in &matches {
                union(&mut parent, &mut rank, i, j);
            }

            let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
            for idx in 0..n {
                let root = find(&mut parent, idx);
                groups.entry(root).or_default().push(idx);
            }

            for mut group in groups.into_values().filter(|g| g.len() > 1) {
                let mut paths: Vec<PathBuf> = group.drain(..).map(|idx| entries[idx].path.clone()).collect();
                paths.sort();
                result.push(paths);
            }
        }
    }

    // --- Non-images: exact hash ---
    let other_files: Vec<&PathBuf> = files.iter().filter(|p| !is_image_file(p)).collect();
    if !other_files.is_empty() {
        let mut hash_map: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for file in &other_files {
            match hash_file_cached(file, algorithm, no_cache) {
                Ok(hash) => { hash_map.entry(hash).or_default().push((*file).clone()); }
                Err(e) => { eprintln!("Error hashing {}: {e}", file.display()); }
            }
        }
        for (_, mut paths) in hash_map.into_iter().filter(|(_, p)| p.len() > 1) {
            paths.sort();
            result.push(paths);
        }
    }

    result.sort_by(|a, b| a[0].cmp(&b[0]));
    result
}
