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
pub enum KeepStrategy { Newest, Oldest, Shallowest, Deepest, First }

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
        if let plist::Value::String(s) = v { !s.starts_with("hash:") && !s.starts_with("hashed:") }
        else { true }
    });
    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    tags.push(plist::Value::String(format!("hash:{tag}")));
    tags.push(plist::Value::String(format!("hashed:{now}")));
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

pub fn select_keep_index(paths: &[&PathBuf], strategy: &KeepStrategy) -> usize {
    match strategy {
        KeepStrategy::Newest => paths.iter().enumerate().max_by_key(|(_, p)| file_mtime(p)).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Oldest => paths.iter().enumerate().min_by_key(|(_, p)| file_mtime(p)).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Shallowest => paths.iter().enumerate().min_by_key(|(_, p)| path_depth(p)).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::Deepest => paths.iter().enumerate().max_by_key(|(_, p)| path_depth(p)).map(|(i, _)| i).unwrap_or(0),
        KeepStrategy::First => paths.iter().enumerate().min_by_key(|(_, p)| p.to_string_lossy().to_string()).map(|(i, _)| i).unwrap_or(0),
    }
}

pub fn prompt_yn(msg: &str) -> bool {
    eprint!("{msg} ");
    io::stderr().flush().ok();
    let mut input = String::new();
    io::stdin().lock().read_line(&mut input).ok();
    matches!(input.trim().to_lowercase().as_str(), "y" | "yes")
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
