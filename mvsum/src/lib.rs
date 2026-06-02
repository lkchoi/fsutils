use clap::ValueEnum;
use std::fs;
use std::io::Read;
use std::path::Path;

#[derive(Clone, ValueEnum)]
pub enum Algorithm {
    Md5,
    Sha1,
    Xxh3,
    Blake3,
}

pub fn hash_file(path: &Path, algo: &Algorithm) -> std::io::Result<String> {
    use sha1::Digest as _;
    let mut file = fs::File::open(path)?;
    let mut buf = [0u8; 8192];

    macro_rules! digest_loop {
        ($hasher:expr, $finalize:expr) => {{
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 { break; }
                $hasher.update(&buf[..n]);
            }
            $finalize
        }};
    }

    Ok(match algo {
        Algorithm::Md5 => {
            let mut ctx = md5::Context::new();
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 { break; }
                ctx.consume(&buf[..n]);
            }
            format!("{:x}", ctx.compute())
        }
        Algorithm::Sha1 => {
            let mut h = sha1::Sha1::new();
            digest_loop!(h, format!("{:x}", h.finalize()))
        }
        Algorithm::Blake3 => {
            let mut h = blake3::Hasher::new();
            digest_loop!(h, h.finalize().to_hex().to_string())
        }
        Algorithm::Xxh3 => {
            let mut h = xxhash_rust::xxh3::Xxh3::new();
            loop {
                let n = file.read(&mut buf)?;
                if n == 0 { break; }
                std::hash::Hasher::write(&mut h, &buf[..n]);
            }
            format!("{:016x}", std::hash::Hasher::finish(&h))
        }
    })
}

pub fn run(paths: &[String], algorithm: &Algorithm, verbose: bool) -> i32 {
    let mut errors = 0i32;
    for src_str in paths {
        let src = Path::new(src_str);
        if src.is_dir() { eprintln!("{src_str} is a directory"); continue; }
        if !src.is_file() { eprintln!("{src_str} is not valid"); errors += 1; continue; }
        if src.file_name().and_then(|n| n.to_str()) == Some(".DS_Store") { continue; }

        let hash = match hash_file(src, algorithm) {
            Ok(h) => h,
            Err(e) => { eprintln!("error: {src_str}: {e}"); errors += 1; continue; }
        };

        let ext = src.extension().and_then(|e| e.to_str()).unwrap_or("");
        let dst = src.with_file_name(if ext.is_empty() { hash.clone() } else { format!("{hash}.{ext}") });

        if let Err(e) = fs::rename(src, &dst) {
            eprintln!("error: {src_str}: {e}"); errors += 1;
        } else if verbose {
            println!("{src_str} -> {}", dst.display());
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn make_temp_file(dir: &TempDir, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, content).unwrap();
        path
    }

    // --- hash_file ---

    #[test]
    fn hash_file_xxh3_deterministic() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, "test.bin", b"hello world");
        let h1 = hash_file(&path, &Algorithm::Xxh3).unwrap();
        let h2 = hash_file(&path, &Algorithm::Xxh3).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 16);
    }

    #[test]
    fn hash_file_md5_known() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, "test.bin", b"hello world");
        let h = hash_file(&path, &Algorithm::Md5).unwrap();
        assert_eq!(h, "5eb63bbbe01eeed093cb22bb8f5acdc3");
    }

    #[test]
    fn hash_file_sha1_known() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, "test.bin", b"hello world");
        let h = hash_file(&path, &Algorithm::Sha1).unwrap();
        assert_eq!(h, "2aae6c35c94fcfb415dbe95f408b9ce91ee846ed");
    }

    #[test]
    fn hash_file_blake3() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, "test.bin", b"hello world");
        let h = hash_file(&path, &Algorithm::Blake3).unwrap();
        assert!(!h.is_empty());
    }

    #[test]
    fn hash_file_different_algos_different_output() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, "test.bin", b"test content");
        let md5 = hash_file(&path, &Algorithm::Md5).unwrap();
        let sha1 = hash_file(&path, &Algorithm::Sha1).unwrap();
        let xxh3 = hash_file(&path, &Algorithm::Xxh3).unwrap();
        assert_ne!(md5, sha1);
        assert_ne!(md5, xxh3);
    }

    // --- run integration ---

    #[test]
    fn run_renames_file_to_hash() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, "original.txt", b"unique content");
        let hash = hash_file(&path, &Algorithm::Xxh3).unwrap();
        let code = run(&[path.to_string_lossy().to_string()], &Algorithm::Xxh3, false);
        assert_eq!(code, 0);
        assert!(!path.exists());
        assert!(dir.path().join(format!("{hash}.txt")).exists());
    }

    #[test]
    fn run_preserves_extension() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, "photo.jpg", b"fake jpeg");
        let code = run(&[path.to_string_lossy().to_string()], &Algorithm::Xxh3, false);
        assert_eq!(code, 0);
        let files: Vec<_> = fs::read_dir(dir.path()).unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with(".jpg"));
    }

    #[test]
    fn run_extensionless_file() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, "noext", b"data");
        let hash = hash_file(&path, &Algorithm::Xxh3).unwrap();
        let code = run(&[path.to_string_lossy().to_string()], &Algorithm::Xxh3, false);
        assert_eq!(code, 0);
        assert!(dir.path().join(&hash).exists());
    }

    #[test]
    fn run_skips_ds_store() {
        let dir = TempDir::new().unwrap();
        let path = make_temp_file(&dir, ".DS_Store", b"ds store data");
        let code = run(&[path.to_string_lossy().to_string()], &Algorithm::Xxh3, false);
        assert_eq!(code, 0);
        assert!(path.exists()); // should not have been renamed
    }

    #[test]
    fn run_directory_is_skipped() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();
        let code = run(&[sub.to_string_lossy().to_string()], &Algorithm::Xxh3, false);
        assert_eq!(code, 0);
    }

    #[test]
    fn run_nonexistent_file_errors() {
        let code = run(&["/nonexistent/file.txt".to_string()], &Algorithm::Xxh3, false);
        assert_ne!(code, 0);
    }
}
