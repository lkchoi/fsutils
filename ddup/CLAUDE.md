# CLAUDE.md

## Project overview

`ddup` is a macOS CLI tool written in Rust. It finds duplicate files using content hashing or perceptual image similarity (SSIM), stores hashes as macOS Finder tags, and can delete duplicates (moved to Trash or hard-deleted).

## Build and run

```sh
cargo build --release
# Binary: target/release/ddup
# Symlinked to: ~/bin/ddup (via install.sh)
```

## Testing

```sh
cargo test              # unit tests
./tests/integration.sh  # integration tests
```

## Architecture

Library crate (`src/lib.rs`) with CLI wrapper (`src/main.rs`).

Key sections in lib.rs:
- **Config** — `load_config()` reads `key=value` files from `~/.config/ddup/config`, `~/.ddup`, `./.ddup`. `apply_config()` injects config values as CLI args before clap parsing. Supports repeatable keys (e.g. `exclude`).
- **Hashing** — `hash_file()` supports md5, sha256, blake3, xxh3 (default)
- **Hash caching** — `hash_file_cached()` checks xattrs (`com.ddup.hash`, `com.ddup.hashed`, `com.ddup.algorithm`) against file mtime before re-hashing
- **Finder tags** — `set_finder_tag()` writes `hash:<value>`, `hashed:<timestamp>`, and `phash:<value>` tags via `com.apple.metadata:_kMDItemUserTags` xattr with binary plist encoding
- **Trash** — `move_to_trash()` uses `osascript` to call Finder's delete (move to trash). `hard_delete()` uses `fs::remove_file`.
- **Exclusion** — `is_excluded()` matches against file/dir name, full path, and individual path components. `compile_excludes()` parses glob patterns. Directories are pruned during walkdir traversal via `filter_entry`.
- **Path resolution** — `resolve_paths()` handles files, directories (with `walkdir`), and glob patterns
- **Keep strategies** — `select_keep_index()` picks which duplicate to keep: best (resolution + format rank), newest, oldest, largest, smallest, shallowest, deepest, first (alphabetical)
- **SSIM duplicate grouping** — `ssim_duplicate_groups()` splits files into images (perceptual similarity via ssim crate) and non-images (exact hash), uses union-find to group matches

## Platform notes

- macOS only.
- Tags set via `com.apple.metadata:_kMDItemUserTags` xattr work directly in Finder.
- Hash caching uses custom `com.ddup.*` xattrs (plain UTF-8 strings, not plist).
- The `hashed:` Finder tag stores a human-readable timestamp; `com.ddup.hashed` xattr stores a unix timestamp for fast cache comparison. Both are set together.

## Dependencies

- `clap` — CLI parsing (derive)
- `md5`, `sha2`, `blake3`, `xxhash-rust` — hash algorithms
- `glob` — glob pattern matching and exclude patterns
- `walkdir` — recursive directory traversal
- `xattr` — extended attribute read/write
- `plist` — binary plist encoding for Finder tags
- `hex` — hex encoding for sha2 output
- `dirs` — home directory and XDG config path resolution
- `chrono` — local time formatting for hashed: tag
- `ssim` — perceptual hashing and SSIM scoring (workspace crate)
- `image` — image loading for SSIM comparison
- `rayon` — parallel processing
