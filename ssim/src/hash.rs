use img_hash::{HasherConfig, HashAlg, ImageHash};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ImageEntry {
    pub path: PathBuf,
    pub hash: ImageHash,
}

pub fn load_and_hash(path: &Path) -> Result<ImageEntry, String> {
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
    let hash = hasher.hash_image(&old_dyn);
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
