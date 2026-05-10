use std::path::Path;
use std::process::Command;

fn mime_to_ext(mime: &str) -> Option<&'static str> {
    match mime {
        "image/jpeg" => Some("jpg"),
        "image/png" => Some("png"),
        "image/gif" => Some("gif"),
        "image/webp" => Some("webp"),
        "image/avif" => Some("avif"),
        "image/heic" => Some("heic"),
        "image/tiff" => Some("tiff"),
        "video/mp4" => Some("mp4"),
        "video/quicktime" => Some("mov"),
        "video/x-matroska" => Some("mkv"),
        "video/webm" => Some("webm"),
        "video/x-msvideo" => Some("avi"),
        "audio/mpeg" => Some("mp3"),
        "audio/mp4" => Some("m4a"),
        "audio/ogg" => Some("ogg"),
        "audio/flac" => Some("flac"),
        "audio/wav" => Some("wav"),
        _ => None,
    }
}

fn detect_mime(path: &Path) -> std::io::Result<String> {
    let out = Command::new("file")
        .args(["-b", "--mime-type", &path.to_string_lossy()])
        .output()?;
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

pub fn run(paths: &[String], dry_run: bool, verbose: bool) -> i32 {
    let mut errors = 0i32;
    for src_str in paths {
        let src = Path::new(src_str);
        if !src.is_file() { eprintln!("not a file: {src_str}"); errors += 1; continue; }

        let mime = match detect_mime(src) {
            Ok(m) => m,
            Err(e) => { eprintln!("error: {src_str}: {e}"); errors += 1; continue; }
        };

        let ext = match mime_to_ext(&mime) {
            Some(e) => e,
            None => { eprintln!("unknown mime: {mime}  {src_str}"); continue; }
        };

        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let dst = src.with_file_name(format!("{stem}.{ext}"));

        if src == dst { continue; }

        if dry_run || verbose {
            println!("{src_str} -> {}", dst.display());
        }
        if !dry_run {
            if let Err(e) = std::fs::rename(src, &dst) {
                eprintln!("error: {src_str}: {e}"); errors += 1;
            }
        }
    }
    errors
}
