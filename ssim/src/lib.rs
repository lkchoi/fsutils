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
