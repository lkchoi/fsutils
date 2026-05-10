use img_hash::{HasherConfig, HashAlg, ImageHash};
use std::path::{Path, PathBuf};

pub struct ImageEntry {
    pub path: PathBuf,
    pub hash: ImageHash,
}

pub fn load_and_hash(path: &Path) -> Result<ImageEntry, String> {
    // Load with image 0.25 (has format support), then convert to img_hash's image 0.23 type
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

pub fn candidate_pairs(entries: &[ImageEntry], threshold: u32) -> Vec<(usize, usize)> {
    let mut pairs = Vec::new();
    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            let dist = entries[i].hash.dist(&entries[j].hash);
            if dist <= threshold {
                pairs.push((i, j));
            }
        }
    }
    pairs
}
