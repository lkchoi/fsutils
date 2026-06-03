use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::SystemTime;

const CWEBP_FORMATS: &[&str] = &["png", "jpg", "jpeg", "tif", "tiff", "webp"];

const SIPS_TO_PNG_FORMATS: &[&str] = &["bmp", "gif", "heic", "heif", "icns", "ico", "psd", "tga"];

fn is_already_webp(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("webp"))
        .unwrap_or(false)
}

fn ext_lower(path: &Path) -> String {
    path.extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
}

fn can_cwebp(ext: &str) -> bool {
    CWEBP_FORMATS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

fn needs_sips_intermediate(ext: &str) -> bool {
    SIPS_TO_PNG_FORMATS.iter().any(|e| e.eq_ignore_ascii_case(ext))
}

fn convert_via_sips(src: &Path, dst: &Path, quality: u8) -> Result<(), String> {
    let tmp_png = dst.with_extension("__tmp__.png");
    let status = Command::new("sips")
        .args(["--setProperty", "format", "png"])
        .arg(src)
        .arg("--out")
        .arg(&tmp_png)
        .output()
        .map_err(|e| format!("sips: {e}"))?;
    if !status.status.success() {
        let _ = fs::remove_file(&tmp_png);
        return Err(format!("sips failed: {}", String::from_utf8_lossy(&status.stderr)));
    }
    let result = run_cwebp(&tmp_png, dst, quality);
    let _ = fs::remove_file(&tmp_png);
    result
}

fn run_cwebp(src: &Path, dst: &Path, quality: u8) -> Result<(), String> {
    let output = Command::new("cwebp")
        .args(["-quiet", "-q"])
        .arg(quality.to_string())
        .arg(src)
        .arg("-o")
        .arg(dst)
        .output()
        .map_err(|e| format!("cwebp: {e}"))?;
    if !output.status.success() {
        return Err(format!("cwebp failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    Ok(())
}

fn copy_xattrs(src: &Path, dst: &Path) -> Result<(), String> {
    let attrs = xattr::list(src).map_err(|e| format!("list xattrs: {e}"))?;
    for name in attrs {
        if let Some(name_str) = name.to_str() {
            if let Ok(Some(val)) = xattr::get(src, name_str) {
                let _ = xattr::set(dst, name_str, &val);
            }
        }
    }
    Ok(())
}

fn copy_times(src: &Path, dst: &Path) -> Result<(), String> {
    let meta = fs::metadata(src).map_err(|e| format!("stat src: {e}"))?;

    // Use touch to set creation time (birth time) — Rust std doesn't expose this
    let created = meta.created().unwrap_or(SystemTime::UNIX_EPOCH);
    let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    let fmt_time = |t: SystemTime| -> String {
        let d = t.duration_since(SystemTime::UNIX_EPOCH).unwrap_or_default();
        d.as_secs().to_string()
    };

    // Set birth time via SetFile -d or touch -t
    // Using touch with explicit timestamps is the most portable macOS approach
    let ctime = fmt_time(created);
    let mtime = fmt_time(modified);

    // Set creation time (birth time) — macOS specific
    let _ = Command::new("touch")
        .args(["-t"])
        .arg(epoch_to_touch(&ctime))
        .arg(dst)
        .output();

    // Set modification time
    let _ = Command::new("touch")
        .args(["-mt"])
        .arg(epoch_to_touch(&mtime))
        .arg(dst)
        .output();

    // Also set birth time via SetFile if available
    let _ = Command::new("SetFile")
        .args(["-d", &epoch_to_setfile(&ctime)])
        .arg(dst)
        .output();

    Ok(())
}

fn epoch_to_touch(epoch: &str) -> String {
    let secs: u64 = epoch.parse().unwrap_or(0);
    let output = Command::new("date")
        .args(["-r", &secs.to_string(), "+%Y%m%d%H%M.%S"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn epoch_to_setfile(epoch: &str) -> String {
    let secs: u64 = epoch.parse().unwrap_or(0);
    let output = Command::new("date")
        .args(["-r", &secs.to_string(), "+%m/%d/%Y %H:%M:%S"])
        .output()
        .unwrap();
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn run(paths: &[String], quality: u8, dry_run: bool, verbose: bool, keep: bool) -> i32 {
    let mut errors = 0i32;
    for src_str in paths {
        let src = Path::new(src_str);
        if !src.is_file() {
            eprintln!("not a file: {src_str}");
            errors += 1;
            continue;
        }

        if is_already_webp(src) {
            if verbose { eprintln!("skip (already webp): {src_str}"); }
            continue;
        }

        let ext = ext_lower(src);
        if !can_cwebp(&ext) && !needs_sips_intermediate(&ext) {
            eprintln!("unsupported format: {src_str}");
            errors += 1;
            continue;
        }

        let dst = src.with_extension("webp");

        if dry_run || verbose {
            println!("{src_str} -> {}", dst.display());
        }
        if dry_run { continue; }

        let result = if needs_sips_intermediate(&ext) {
            convert_via_sips(src, &dst, quality)
        } else {
            run_cwebp(src, &dst, quality)
        };

        if let Err(e) = result {
            eprintln!("error: {src_str}: {e}");
            errors += 1;
            continue;
        }

        if let Err(e) = copy_xattrs(src, &dst) {
            eprintln!("warning: {src_str}: {e}");
        }
        if let Err(e) = copy_times(src, &dst) {
            eprintln!("warning: {src_str}: {e}");
        }

        if !keep {
            if let Err(e) = fs::remove_file(src) {
                eprintln!("error removing original: {src_str}: {e}");
                errors += 1;
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_already_webp_variants() {
        assert!(is_already_webp(Path::new("file.webp")));
        assert!(is_already_webp(Path::new("file.WEBP")));
        assert!(is_already_webp(Path::new("file.WebP")));
        assert!(!is_already_webp(Path::new("file.png")));
        assert!(!is_already_webp(Path::new("noext")));
    }

    #[test]
    fn can_cwebp_known() {
        assert!(can_cwebp("png"));
        assert!(can_cwebp("jpg"));
        assert!(can_cwebp("jpeg"));
        assert!(can_cwebp("tiff"));
        assert!(!can_cwebp("bmp"));
        assert!(!can_cwebp("gif"));
    }

    #[test]
    fn needs_sips_known() {
        assert!(needs_sips_intermediate("bmp"));
        assert!(needs_sips_intermediate("gif"));
        assert!(needs_sips_intermediate("heic"));
        assert!(needs_sips_intermediate("psd"));
        assert!(!needs_sips_intermediate("png"));
        assert!(!needs_sips_intermediate("jpg"));
    }

    #[test]
    fn skip_already_webp() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("photo.webp");
        fs::write(&file, "fake").unwrap();
        let code = run(&[file.to_string_lossy().to_string()], 80, false, false, true);
        assert_eq!(code, 0);
        assert!(file.exists());
    }

    #[test]
    fn unsupported_format() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("doc.pdf");
        fs::write(&file, "fake").unwrap();
        let code = run(&[file.to_string_lossy().to_string()], 80, false, false, true);
        assert_ne!(code, 0);
    }

    #[test]
    fn not_a_file() {
        let code = run(&["/nonexistent/file.png".to_string()], 80, false, false, true);
        assert_ne!(code, 0);
    }

    #[test]
    fn dry_run_does_not_convert() {
        let dir = tempfile::TempDir::new().unwrap();
        let file = dir.path().join("photo.png");
        fs::write(&file, "fake png").unwrap();
        let code = run(&[file.to_string_lossy().to_string()], 80, true, false, true);
        assert_eq!(code, 0);
        assert!(file.exists());
        assert!(!dir.path().join("photo.webp").exists());
    }
}
