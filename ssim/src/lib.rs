pub mod hash;
pub mod ssim;

use std::collections::HashMap;
use std::path::PathBuf;
use rayon::prelude::*;

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "bmp", "gif", "webp", "tiff", "tif"];

fn is_image(path: &PathBuf) -> bool {
    path.extension().map_or(false, |ext| {
        IMAGE_EXTENSIONS.contains(&ext.to_string_lossy().to_lowercase().as_str())
    })
}

pub fn resolve_paths(inputs: &[String]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for input in inputs {
        let p = PathBuf::from(input);
        if p.is_dir() {
            // Directory: scan for images
            if let Ok(entries) = std::fs::read_dir(&p) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() && is_image(&path) {
                        paths.push(path);
                    }
                }
            } else {
                eprintln!("Error reading directory: {}", p.display());
            }
        } else if p.is_file() {
            // Explicit file: use directly (even if not an image extension — let image crate decide)
            paths.push(p);
        } else {
            // Glob pattern
            match glob::glob(input) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(path) if path.is_file() && is_image(&path) => paths.push(path),
                            Ok(_) => {}
                            Err(e) => eprintln!("Warning: {}", e),
                        }
                    }
                }
                Err(e) => eprintln!("Invalid glob pattern '{}': {}", input, e),
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

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

pub struct SimilarMatch {
    pub path: PathBuf,
    pub score: f64,
}

pub fn find_similar(reference: &str, dirs: &[String], threshold: f64, hash_threshold: u32) -> Result<Vec<SimilarMatch>, String> {
    let ref_path = PathBuf::from(reference);
    if !ref_path.is_file() {
        return Err(format!("{}: not a file", reference));
    }

    let ref_entry = hash::load_and_hash(&ref_path)?;

    let candidates = resolve_paths_recursive(dirs);
    if candidates.is_empty() {
        return Ok(Vec::new());
    }

    // Hash all candidates (ref image at index 0, candidates starting at index 1)
    let mut entries: Vec<hash::ImageEntry> = vec![ref_entry];
    let hashed: Vec<hash::ImageEntry> = candidates.par_iter()
        .filter(|p| *p != &PathBuf::from(reference).canonicalize().unwrap_or_else(|_| PathBuf::from(reference)))
        .filter_map(|p| match hash::load_and_hash(p) {
            Ok(entry) => Some(entry),
            Err(e) => { eprintln!("Warning: {}", e); None }
        })
        .collect();
    entries.extend(hashed);

    if entries.len() <= 1 {
        return Ok(Vec::new());
    }

    // Find candidates close to the reference (index 0) via hamming distance
    let mut matches: Vec<SimilarMatch> = Vec::new();
    let ref_img = image::open(&entries[0].path).map_err(|e| format!("{}: {}", entries[0].path.display(), e))?;

    let similar: Vec<(usize, f64)> = (1..entries.len()).into_par_iter()
        .filter(|&i| entries[0].hash.dist(&entries[i].hash) <= hash_threshold)
        .filter_map(|i| {
            let img = image::open(&entries[i].path).ok()?;
            let score = ssim::compute_ssim(&ref_img, &img);
            if score >= threshold { Some((i, score)) } else { None }
        })
        .collect();

    for (i, score) in similar {
        matches.push(SimilarMatch {
            path: entries[i].path.clone(),
            score,
        });
    }

    matches.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    Ok(matches)
}

fn resolve_paths_recursive(inputs: &[String]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = Vec::new();
    for input in inputs {
        let p = PathBuf::from(input);
        if p.is_dir() {
            collect_images_recursive(&p, &mut paths);
        } else if p.is_file() {
            paths.push(p);
        } else {
            match glob::glob(input) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(path) if path.is_file() && is_image(&path) => paths.push(path),
                            Ok(_) => {}
                            Err(e) => eprintln!("Warning: {}", e),
                        }
                    }
                }
                Err(e) => eprintln!("Invalid glob pattern '{}': {}", input, e),
            }
        }
    }
    paths.sort();
    paths.dedup();
    paths
}

fn collect_images_recursive(dir: &PathBuf, paths: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                collect_images_recursive(&path, paths);
            } else if path.is_file() && is_image(&path) {
                paths.push(path);
            }
        }
    }
}

pub fn run(inputs: &[String], threshold: f64, hash_threshold: u32) -> i32 {
    let paths = resolve_paths(inputs);
    if paths.is_empty() { eprintln!("No images found matching the given inputs"); return 0; }

    let entries: Vec<hash::ImageEntry> = paths.par_iter()
        .filter_map(|p| match hash::load_and_hash(p) {
            Ok(entry) => Some(entry),
            Err(e) => { eprintln!("Warning: {}", e); None }
        })
        .collect();

    let candidates = hash::candidate_pairs(&entries, hash_threshold);
    if candidates.is_empty() { return 0; }

    let matches: Vec<(usize, usize, f64)> = candidates.par_iter()
        .filter_map(|&(i, j)| {
            let img_a = image::open(&entries[i].path).ok()?;
            let img_b = image::open(&entries[j].path).ok()?;
            let score = ssim::compute_ssim(&img_a, &img_b);
            if score >= threshold { Some((i, j, score)) } else { None }
        })
        .collect();

    if matches.is_empty() { return 0; }

    let n = entries.len();
    let mut parent: Vec<usize> = (0..n).collect();
    let mut rank = vec![0usize; n];
    let mut scores: HashMap<(usize, usize), f64> = HashMap::new();

    for &(i, j, score) in &matches {
        union(&mut parent, &mut rank, i, j);
        scores.insert((i, j), score);
        scores.insert((j, i), score);
    }

    let mut groups: HashMap<usize, Vec<usize>> = HashMap::new();
    for idx in 0..n {
        let root = find(&mut parent, idx);
        groups.entry(root).or_default().push(idx);
    }

    let mut dup_groups: Vec<Vec<usize>> = groups.into_values().filter(|g| g.len() > 1).collect();
    dup_groups.sort_by_key(|g| g[0]);

    for group in &dup_groups {
        let mut min_score = f64::MAX;
        for (k, &a) in group.iter().enumerate() {
            for &b in &group[k + 1..] {
                if let Some(&s) = scores.get(&(a, b)).or_else(|| scores.get(&(b, a))) {
                    if s < min_score { min_score = s; }
                }
            }
        }
        let mut file_names: Vec<String> = group.iter()
            .map(|&idx| entries[idx].path.display().to_string())
            .collect();
        file_names.sort();
        println!("{:.4} {}", min_score, file_names.join(" "));
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    // --- is_image ---

    #[test]
    fn is_image_known_extensions() {
        assert!(is_image(&PathBuf::from("photo.jpg")));
        assert!(is_image(&PathBuf::from("photo.JPEG")));
        assert!(is_image(&PathBuf::from("photo.png")));
        assert!(is_image(&PathBuf::from("photo.webp")));
        assert!(is_image(&PathBuf::from("photo.tiff")));
    }

    #[test]
    fn is_image_rejects_non_images() {
        assert!(!is_image(&PathBuf::from("doc.pdf")));
        assert!(!is_image(&PathBuf::from("video.mp4")));
        assert!(!is_image(&PathBuf::from("noext")));
    }

    // --- union-find ---

    #[test]
    fn union_find_basic() {
        let mut parent: Vec<usize> = (0..5).collect();
        let mut rank = vec![0usize; 5];

        union(&mut parent, &mut rank, 0, 1);
        union(&mut parent, &mut rank, 2, 3);
        assert_eq!(find(&mut parent, 0), find(&mut parent, 1));
        assert_ne!(find(&mut parent, 0), find(&mut parent, 2));

        union(&mut parent, &mut rank, 1, 3);
        assert_eq!(find(&mut parent, 0), find(&mut parent, 3));
    }

    #[test]
    fn union_find_self() {
        let mut parent: Vec<usize> = (0..3).collect();
        let mut rank = vec![0usize; 3];
        union(&mut parent, &mut rank, 0, 0);
        assert_eq!(find(&mut parent, 0), 0);
    }

    // --- resolve_paths ---

    #[test]
    fn resolve_paths_finds_images_in_dir() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("a.jpg"), "img").unwrap();
        fs::write(dir.path().join("b.png"), "img").unwrap();
        fs::write(dir.path().join("c.txt"), "txt").unwrap();
        let paths = resolve_paths(&[dir.path().to_string_lossy().to_string()]);
        assert_eq!(paths.len(), 2); // only images
    }

    #[test]
    fn resolve_paths_explicit_file_any_ext() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("data.bin");
        fs::write(&file, "binary").unwrap();
        let paths = resolve_paths(&[file.to_string_lossy().to_string()]);
        assert_eq!(paths.len(), 1); // explicit files always included
    }

    #[test]
    fn resolve_paths_deduplicates() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("a.jpg");
        fs::write(&file, "img").unwrap();
        let path_str = file.to_string_lossy().to_string();
        let paths = resolve_paths(&[path_str.clone(), path_str]);
        assert_eq!(paths.len(), 1);
    }

    #[test]
    fn resolve_paths_empty_input() {
        let paths = resolve_paths(&[]);
        assert!(paths.is_empty());
    }
}
