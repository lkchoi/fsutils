# ssim

Find near-duplicate images in a directory using perceptual hashing (for fast pre-filtering) and SSIM (for accurate similarity scoring).

## Usage

```sh
ssim [OPTIONS] <DIRECTORY>
```

## Options

| Flag | Description |
|------|-------------|
| `--threshold <THRESHOLD>` | SSIM threshold for duplicates, 0.0-1.0 (default: 0.95) |
| `--hash-threshold <DISTANCE>` | Max Hamming distance for hash pre-filter, 0-64 (default: 10) |

## Output

One line per group of similar images: the minimum pairwise SSIM score followed by space-separated file paths.

```
0.9823 ./IMG_001.jpg ./IMG_002.jpg
0.9912 ./photo_a.png ./photo_b.png ./photo_c.png
```

## Examples

```sh
# Find near-duplicates with default settings
ssim /path/to/images

# More permissive matching
ssim --threshold 0.90 --hash-threshold 15 /path/to/files

# Strict matching (nearly identical only)
ssim --threshold 0.99 /path/to/images
```

## How It Works

1. Scans directory for image files (jpg, jpeg, png, bmp, gif, webp, tiff)
2. Computes perceptual hashes in parallel
3. Finds candidate pairs by Hamming distance (fast pre-filter)
4. Computes SSIM on candidate pairs in parallel
5. Groups matches using union-find and outputs results

## Notes

- Only scans the top-level directory (not recursive)
- Uses parallel processing via rayon for performance
- Lower threshold = more permissive matching
- Higher hash-threshold = more candidates checked (slower but catches more)
