# ssim

Find near-duplicate images using perceptual hashing (for fast pre-filtering) and SSIM (for accurate similarity scoring).

## Usage

```sh
ssim [OPTIONS] <PATHS>...
```

Each positional argument can be a file, directory, or glob pattern:

- **Directory** — scans for image files within it
- **File** — used directly
- **Glob pattern** — expanded and filtered to image files

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
# Scan a directory
ssim /path/to/images

# Specific files
ssim /path/to/a.jpg /path/to/b.png

# Glob patterns
ssim "/path/to/photos/**/*.jpg" "/path/to/images/*.png"

# Mix of directories, files, and globs
ssim /path/to/images /path/to/extra.jpg "/path/to/more/**/*.webp"

# More permissive matching
ssim --threshold 0.90 --hash-threshold 15 /path/to/files

# Strict matching (nearly identical only)
ssim --threshold 0.99 /path/to/images
```

## How It Works

1. Resolves inputs: directories are scanned, files are used directly, globs are expanded
2. Filters for image files (jpg, jpeg, png, bmp, gif, webp, tiff)
3. Computes perceptual hashes in parallel
4. Finds candidate pairs by Hamming distance (fast pre-filter)
5. Computes SSIM on candidate pairs in parallel
6. Groups matches using union-find and outputs results

## Notes

- Directory scanning is not recursive — use globs like `**/*.jpg` for recursive matching
- Uses parallel processing via rayon for performance
- Lower threshold = more permissive matching
- Higher hash-threshold = more candidates checked (slower but catches more)
