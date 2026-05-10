# CLAUDE.md

## Project overview

`ddup` is a macOS CLI tool written in Rust. It hashes files and stores the hash as a macOS Finder tag. It also detects and deletes duplicate files (moved to Trash).

## Build and run

```sh
cargo build --release
# Binary: target/release/ddup
# Symlinked to: /usr/local/bin/ddup
```

## Testing

```sh
cargo test              # unit tests (21)
./tests/integration.sh  # integration tests (24)
```

## Architecture

Single-file project: `src/main.rs`. No modules or library crate.

Key sections in main.rs:
- **Config** — `load_config()` reads `key=value` files from `~/.config/ddup/config`, `~/.ddup`, `./.ddup`. `apply_config()` injects config values as CLI args before clap parsing. Supports repeatable keys (e.g. `exclude`).
- **Hashing** — `hash_file()` supports md5, sha256, blake3, xxh3
- **Hash caching** — `hash_file_cached()` checks xattrs (`com.ddup.hash`, `com.ddup.hashed`, `com.ddup.algorithm`) against file mtime before re-hashing
- **Finder tags** — `set_finder_tag()` writes `hash:<value>` and `hashed:<timestamp>` tags via `com.apple.metadata:_kMDItemUserTags` xattr with binary plist encoding
- **Trash** — `move_to_trash()` uses `osascript` to call Finder's delete (move to trash)
- **Exclusion** — `is_excluded()` matches against file/dir name, full path, and individual path components. `compile_excludes()` parses glob patterns. Directories are pruned during walkdir traversal via `filter_entry`.
- **Path resolution** — `resolve_paths()` handles files, directories (with `walkdir`), and glob patterns
- **Duplicate deletion** — interactive mode (user picks which file to keep) or strategy mode (`--keep` flag). Strategies: newest, oldest, shallowest, deepest, first.

## Platform notes

- macOS only.
- Tags set via `com.apple.metadata:_kMDItemUserTags` xattr work directly in Finder.
- Finder comments (removed in favor of tags) required `osascript` and only worked on Spotlight-indexed volumes.
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
