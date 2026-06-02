use img_hash::{HasherConfig, HashAlg, ImageHash};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

pub struct ImageEntry {
    pub path: PathBuf,
    pub hash: ImageHash,
}

const XATTR_PHASH: &str = "com.ssim.phash";
const XATTR_PHASH_AT: &str = "com.ssim.phashed";

fn get_cached_phash(path: &Path) -> Option<ImageHash> {
    let ts_data = xattr::get(path, XATTR_PHASH_AT).ok()??;
    let ts_str = String::from_utf8(ts_data).ok()?;
    let hashed_at: u64 = ts_str.parse().ok()?;
    let mtime = std::fs::metadata(path).ok()?.modified().ok()?
        .duration_since(SystemTime::UNIX_EPOCH).ok()?.as_secs();
    if mtime > hashed_at { return None; }
    let hash_data = xattr::get(path, XATTR_PHASH).ok()??;
    let hex_str = String::from_utf8(hash_data).ok()?;
    let bytes = hex::decode(&hex_str).ok()?;
    ImageHash::from_bytes(&bytes).ok()
}

fn set_phash_cache(path: &Path, hash: &ImageHash) {
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs().to_string();
    let _ = xattr::set(path, XATTR_PHASH, hex::encode(hash.as_bytes()).as_bytes());
    let _ = xattr::set(path, XATTR_PHASH_AT, now.as_bytes());
}

fn compute_phash(path: &Path) -> Result<ImageHash, String> {
    let img = image::open(path).map_err(|e| format!("{}: {}", path.display(), e))?;
    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let old_img: img_hash::image::RgbaImage =
        img_hash::image::ImageBuffer::from_raw(w, h, rgba.into_raw())
            .ok_or_else(|| format!("{}: failed to convert image", path.display()))?;
    let old_dyn = img_hash::image::DynamicImage::ImageRgba8(old_img);

    let hasher = HasherConfig::new()
        .hash_size(8, 8)
        .hash_alg(HashAlg::Gradient)
        .to_hasher();
    Ok(hasher.hash_image(&old_dyn))
}

pub fn load_and_hash(path: &Path) -> Result<ImageEntry, String> {
    let hash = if let Some(cached) = get_cached_phash(path) {
        cached
    } else {
        let h = compute_phash(path)?;
        set_phash_cache(path, &h);
        h
    };
    Ok(ImageEntry {
        path: path.to_owned(),
        hash,
    })
}

// --- BK-tree for Hamming distance ---

struct BkNode {
    index: usize,
    children: HashMap<u32, BkNode>,
}

struct BkTree {
    root: Option<BkNode>,
}

impl BkTree {
    fn new() -> Self {
        BkTree { root: None }
    }

    fn insert(&mut self, index: usize, entries: &[ImageEntry]) {
        match self.root {
            None => {
                self.root = Some(BkNode { index, children: HashMap::new() });
            }
            Some(ref mut root) => {
                Self::insert_node(root, index, entries);
            }
        }
    }

    fn insert_node(node: &mut BkNode, index: usize, entries: &[ImageEntry]) {
        let dist = entries[node.index].hash.dist(&entries[index].hash);
        match node.children.get_mut(&dist) {
            Some(child) => Self::insert_node(child, index, entries),
            None => { node.children.insert(dist, BkNode { index, children: HashMap::new() }); }
        }
    }

    fn query(&self, query_index: usize, threshold: u32, entries: &[ImageEntry], results: &mut Vec<(usize, usize)>) {
        if let Some(ref root) = self.root {
            Self::query_node(root, query_index, threshold, entries, results);
        }
    }

    fn query_node(node: &BkNode, query_index: usize, threshold: u32, entries: &[ImageEntry], results: &mut Vec<(usize, usize)>) {
        let dist = entries[node.index].hash.dist(&entries[query_index].hash);
        if dist <= threshold && node.index < query_index {
            results.push((node.index, query_index));
        }
        let lo = dist.saturating_sub(threshold);
        let hi = dist + threshold;
        for (&edge_dist, child) in &node.children {
            if edge_dist >= lo && edge_dist <= hi {
                Self::query_node(child, query_index, threshold, entries, results);
            }
        }
    }
}

pub fn candidate_pairs(entries: &[ImageEntry], threshold: u32) -> Vec<(usize, usize)> {
    let mut tree = BkTree::new();
    let mut pairs = Vec::new();

    for i in 0..entries.len() {
        tree.query(i, threshold, entries, &mut pairs);
        tree.insert(i, entries);
    }

    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(path: &str, hash_bytes: &[u8]) -> ImageEntry {
        ImageEntry {
            path: PathBuf::from(path),
            hash: ImageHash::from_bytes(hash_bytes).unwrap(),
        }
    }

    // --- BK-tree ---

    #[test]
    fn bk_tree_empty() {
        let entries: Vec<ImageEntry> = vec![];
        let pairs = candidate_pairs(&entries, 10);
        assert!(pairs.is_empty());
    }

    #[test]
    fn bk_tree_single_entry() {
        let entries = vec![make_entry("a.jpg", &[0u8; 8])];
        let pairs = candidate_pairs(&entries, 10);
        assert!(pairs.is_empty());
    }

    #[test]
    fn bk_tree_identical_hashes_are_candidates() {
        let entries = vec![
            make_entry("a.jpg", &[0u8; 8]),
            make_entry("b.jpg", &[0u8; 8]),
        ];
        let pairs = candidate_pairs(&entries, 0);
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0], (0, 1));
    }

    #[test]
    fn bk_tree_distant_hashes_not_candidates() {
        let entries = vec![
            make_entry("a.jpg", &[0x00; 8]),
            make_entry("b.jpg", &[0xFF; 8]),
        ];
        let pairs = candidate_pairs(&entries, 5);
        assert!(pairs.is_empty());
    }

    #[test]
    fn bk_tree_respects_threshold() {
        // Two hashes that differ by exactly 1 bit
        let entries = vec![
            make_entry("a.jpg", &[0b0000_0000, 0, 0, 0, 0, 0, 0, 0]),
            make_entry("b.jpg", &[0b0000_0001, 0, 0, 0, 0, 0, 0, 0]),
        ];
        // threshold=0 should not match
        assert!(candidate_pairs(&entries, 0).is_empty());
        // threshold=1 should match
        assert_eq!(candidate_pairs(&entries, 1).len(), 1);
    }

    #[test]
    fn bk_tree_multiple_groups() {
        let entries = vec![
            make_entry("a.jpg", &[0x00; 8]),
            make_entry("b.jpg", &[0x00; 8]),
            make_entry("c.jpg", &[0xFF; 8]),
            make_entry("d.jpg", &[0xFF; 8]),
        ];
        let pairs = candidate_pairs(&entries, 0);
        assert_eq!(pairs.len(), 2); // (0,1) and (2,3)
    }

    // --- load_and_hash with real temp image ---

    #[test]
    fn load_and_hash_small_image() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("test.png");
        // Create a small valid PNG
        let img = image::RgbaImage::from_fn(16, 16, |x, y| {
            image::Rgba([(x * 16) as u8, (y * 16) as u8, 128, 255])
        });
        img.save(&path).unwrap();

        let entry = load_and_hash(&path).unwrap();
        assert_eq!(entry.path, path);
        assert!(!entry.hash.as_bytes().is_empty());
    }

    #[test]
    fn load_and_hash_caches_phash() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("cached.png");
        let img = image::RgbaImage::from_fn(16, 16, |_, _| image::Rgba([100, 100, 100, 255]));
        img.save(&path).unwrap();

        let entry1 = load_and_hash(&path).unwrap();
        // Second call should use cache
        let entry2 = load_and_hash(&path).unwrap();
        assert_eq!(entry1.hash.as_bytes(), entry2.hash.as_bytes());

        // Verify xattr was set
        let phash_data = xattr::get(&path, XATTR_PHASH).unwrap();
        assert!(phash_data.is_some());
    }
}
