use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI32, Ordering};

pub fn run(src: &Path, target: &Path, length: usize, copy: bool, dry_run: bool) -> i32 {
    let errors = AtomicI32::new(0);

    // Collect files to bucket (skip dotfiles and _-prefixed files)
    let files: Vec<PathBuf> = match fs::read_dir(src) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_type().map_or(false, |ft| ft.is_file())
                    && e.file_name().to_string_lossy().len() >= length
                    && !e.file_name().to_string_lossy().starts_with('.')
                    && !e.file_name().to_string_lossy().starts_with('_')
            })
            .map(|e| e.path())
            .collect(),
        Err(e) => {
            eprintln!("error: {}: {e}", src.display());
            return 1;
        }
    };

    if files.is_empty() {
        eprintln!("No files found.");
        return 0;
    }

    let action = if copy { "copy" } else { "move" };
    let pb = ProgressBar::new(files.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template("{msg} [{bar:40}] {pos}/{len}")
            .unwrap()
            .progress_chars("=> "),
    );
    pb.set_message(action.to_string());

    files.par_iter().for_each(|file| {
        let name = file.file_name().unwrap().to_string_lossy();
        let prefix = &name[..length];
        let dest_dir = target.join(prefix);

        if dry_run {
            println!("{} -> {}/{}", file.display(), dest_dir.display(), name);
            pb.inc(1);
            return;
        }

        if let Err(e) = fs::create_dir_all(&dest_dir) {
            eprintln!("error: mkdir {}: {e}", dest_dir.display());
            errors.fetch_add(1, Ordering::Relaxed);
            pb.inc(1);
            return;
        }

        let dest = dest_dir.join(&*name);
        let result = if copy {
            fs::copy(file, &dest).map(|_| ())
        } else {
            fs::rename(file, &dest)
        };

        if let Err(e) = result {
            eprintln!("error: {} -> {}: {e}", file.display(), dest.display());
            errors.fetch_add(1, Ordering::Relaxed);
        }
        pb.inc(1);
    });

    pb.finish_and_clear();

    let errs = errors.load(Ordering::Relaxed);

    // Set dir counts on target
    let dirs: Vec<PathBuf> = match fs::read_dir(target) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().map_or(false, |ft| ft.is_dir()))
            .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
            .map(|e| e.path())
            .collect(),
        Err(_) => Vec::new(),
    };

    if !dry_run && !dirs.is_empty() {
        errs + dss::run_dir_count(&dirs, false)
    } else {
        errs
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn copy_into_prefix_dirs() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("aabb.txt"), "a").unwrap();
        fs::write(dir.path().join("aacc.txt"), "b").unwrap();
        fs::write(dir.path().join("bbdd.txt"), "c").unwrap();
        let code = run(dir.path(), dir.path(), 2, true, false);
        assert_eq!(code, 0);
        assert!(dir.path().join("aa/aabb.txt").exists());
        assert!(dir.path().join("aa/aacc.txt").exists());
        assert!(dir.path().join("bb/bbdd.txt").exists());
        // originals still exist (copy mode)
        assert!(dir.path().join("aabb.txt").exists());
    }

    #[test]
    fn move_into_prefix_dirs() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("aabb.txt"), "a").unwrap();
        fs::write(dir.path().join("bbdd.txt"), "b").unwrap();
        let code = run(dir.path(), dir.path(), 2, false, false);
        assert_eq!(code, 0);
        assert!(dir.path().join("aa/aabb.txt").exists());
        assert!(dir.path().join("bb/bbdd.txt").exists());
        // originals removed (move mode)
        assert!(!dir.path().join("aabb.txt").exists());
    }

    #[test]
    fn custom_length() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("abcdef.txt"), "a").unwrap();
        let code = run(dir.path(), dir.path(), 3, true, false);
        assert_eq!(code, 0);
        assert!(dir.path().join("abc/abcdef.txt").exists());
    }

    #[test]
    fn separate_target() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        fs::write(src.path().join("aabb.txt"), "a").unwrap();
        let code = run(src.path(), dst.path(), 2, true, false);
        assert_eq!(code, 0);
        assert!(dst.path().join("aa/aabb.txt").exists());
    }

    #[test]
    fn skips_dotfiles_and_underscored() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".hidden"), "h").unwrap();
        fs::write(dir.path().join("_list.csv"), "l").unwrap();
        fs::write(dir.path().join("aabb.txt"), "a").unwrap();
        let code = run(dir.path(), dir.path(), 2, true, false);
        assert_eq!(code, 0);
        assert!(dir.path().join("aa/aabb.txt").exists());
        // skipped files not bucketed
        assert!(!dir.path().join(".h").exists());
        assert!(!dir.path().join("_l").exists());
    }

    #[test]
    fn dry_run_does_not_create() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("aabb.txt"), "a").unwrap();
        let code = run(dir.path(), dir.path(), 2, true, true);
        assert_eq!(code, 0);
        assert!(!dir.path().join("aa").exists());
    }
}
