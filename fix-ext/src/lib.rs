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
        "image/bmp" | "image/x-ms-bmp" => Some("bmp"),
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

fn normalize_ext(ext: &str) -> String {
    let lower = ext.to_lowercase();
    match lower.as_str() {
        "jpeg" => "jpg".to_string(),
        _ => lower,
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

        let current_ext = match src.extension().and_then(|s| s.to_str()) {
            Some(e) if !e.is_empty() => e,
            _ => continue,
        };

        // MIME-based correction takes priority, otherwise normalize current extension
        let ext = match detect_mime(src) {
            Ok(mime) => match mime_to_ext(&mime) {
                Some(e) => e.to_string(),
                None => normalize_ext(current_ext),
            },
            Err(_) => normalize_ext(current_ext),
        };

        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        let dst = src.with_file_name(format!("{stem}.{ext}"));

        if src == dst { continue; }

        if dry_run || verbose {
            println!("{src_str} -> {}", dst.display());
        }
        if !dry_run {
            // On case-insensitive filesystems (macOS APFS/HFS+), a direct
            // rename("file.JPG", "file.jpg") can silently no-op. Route through
            // an intermediate name to force the case change.
            let result = if src.to_string_lossy().to_lowercase() == dst.to_string_lossy().to_lowercase() {
                let orig_name = src.file_name().unwrap().to_string_lossy();
                let tmp = src.with_file_name(format!("{orig_name}.tmp"));
                std::fs::rename(src, &tmp).and_then(|_| {
                    std::fs::rename(&tmp, &dst).or_else(|e| {
                        let _ = std::fs::rename(&tmp, src); // restore on failure
                        Err(e)
                    })
                })
            } else {
                std::fs::rename(src, &dst)
            };
            if let Err(e) = result {
                eprintln!("error: {src_str}: {e}"); errors += 1;
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- mime_to_ext ---

    #[test]
    fn mime_to_ext_known_types() {
        assert_eq!(mime_to_ext("image/jpeg"), Some("jpg"));
        assert_eq!(mime_to_ext("image/png"), Some("png"));
        assert_eq!(mime_to_ext("video/mp4"), Some("mp4"));
        assert_eq!(mime_to_ext("audio/mpeg"), Some("mp3"));
        assert_eq!(mime_to_ext("image/heic"), Some("heic"));
    }

    #[test]
    fn mime_to_ext_unknown_returns_none() {
        assert_eq!(mime_to_ext("application/pdf"), None);
        assert_eq!(mime_to_ext("text/plain"), None);
    }

    // --- normalize_ext ---

    #[test]
    fn normalize_ext_lowercases() {
        assert_eq!(normalize_ext("PNG"), "png");
        assert_eq!(normalize_ext("Mp4"), "mp4");
    }

    #[test]
    fn normalize_ext_jpeg_to_jpg() {
        assert_eq!(normalize_ext("jpeg"), "jpg");
        assert_eq!(normalize_ext("JPEG"), "jpg");
    }

    #[test]
    fn normalize_ext_passthrough() {
        assert_eq!(normalize_ext("webp"), "webp");
    }

    // --- run integration ---

    #[test]
    fn run_dry_run_does_not_rename() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("photo.JPEG");
        std::fs::write(&file, "not a real jpeg").unwrap();
        let code = run(&[file.to_string_lossy().to_string()], true, false);
        assert_eq!(code, 0);
        assert!(file.exists());
    }

    #[test]
    fn run_normalizes_case() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("photo.JPEG");
        std::fs::write(&file, "not a real jpeg").unwrap();
        let code = run(&[file.to_string_lossy().to_string()], false, false);
        assert_eq!(code, 0);
        // Should have been renamed to photo.jpg (normalize_ext: JPEG -> jpg)
        // Note: detect_mime may classify as text/plain, so mime_to_ext returns None,
        // falling back to normalize_ext("JPEG") -> "jpg"
        assert!(dir.path().join("photo.jpg").exists());
    }

    #[test]
    fn run_skips_extensionless_files() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("noext");
        std::fs::write(&file, "data").unwrap();
        let code = run(&[file.to_string_lossy().to_string()], false, false);
        assert_eq!(code, 0);
        assert!(file.exists());
    }

    #[test]
    fn run_already_correct_is_noop() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("file.txt");
        std::fs::write(&file, "hello").unwrap();
        let code = run(&[file.to_string_lossy().to_string()], false, false);
        assert_eq!(code, 0);
        assert!(file.exists());
    }

    #[test]
    fn run_not_a_file_returns_error() {
        let code = run(&["/nonexistent/path.jpg".to_string()], false, false);
        assert_ne!(code, 0);
    }
}
