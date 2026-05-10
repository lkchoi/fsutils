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
