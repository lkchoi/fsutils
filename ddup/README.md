# ddup

A macOS CLI tool that finds duplicate files using content hashing or perceptual image similarity (SSIM). Stores hashes as Finder tags and can delete duplicates.

## Install

```sh
# From workspace root:
./install.sh

# Or standalone:
cargo install --path .
```

## Usage

```
ddup [OPTIONS] <PATHS>...
```

Accepts file paths, directories (recursive by default), and glob patterns.

### Find and delete duplicates

By default, ddup finds duplicates using perceptual similarity for images (SSIM) and exact hashing for other files, then prompts to delete them:

```sh
# Find and manage duplicates (interactive)
ddup /path/to/files

# Auto-select which to keep, confirm at the end
ddup --keep=best /path/to/files

# Skip confirmation — move to Trash
ddup --keep=newest --yes /path/to/files

# Skip confirmation — permanently delete
ddup --keep=oldest --hard /path/to/files

# Dry run — show duplicates without deleting or tagging
ddup -n /path/to/files
```

### Hash-only mode (no SSIM)

```sh
# Use exact hash matching only
ddup --ssim=false /path/to/files

# Use a specific algorithm
ddup -a blake3 /path/to/files
```

Each file gets Finder tags: `hash:<value>`, `hashed:<timestamp>`, and `phash:<value>` (for images).

### Keep strategies

| Strategy | Keeps |
|---|---|
| `best` (default) | Highest resolution, then best format (webp > png > tiff > jpg > gif > bmp) |
| `newest` | Most recently modified file |
| `oldest` | Earliest modified file |
| `largest` | Largest file by size |
| `smallest` | Smallest file by size |
| `shallowest` | File with fewest path components |
| `deepest` | File with most path components |
| `first` | First file alphabetically |

### Exclude patterns

```sh
# Exclude directories or file patterns
ddup -e node_modules -e "*.log" /path/to/project

# Multiple excludes can also be set in config
```

### Hash caching

Hashes are cached in extended attributes (`com.ddup.hash`, `com.ddup.hashed`, `com.ddup.algorithm`). On subsequent runs, files whose modification time hasn't changed use the cached hash.

```sh
# Force re-hash all files
ddup --no-cache /path/to/files
```

### Config file

Config is loaded from three paths (later overrides earlier):

1. `~/.config/ddup/config` (XDG, respects `$XDG_CONFIG_HOME`)
2. `~/.ddup` (home dotfile)
3. `./.ddup` (local/project)

Simple `key=value` format, `#` for comments. Keys match CLI flag names:

```
# example config
algorithm=blake3
exclude=node_modules
exclude=.git
```

CLI flags always override config values.

## Algorithms

| Algorithm | Flag | Output length | Notes |
|---|---|---|---|
| XXH3 | `-a xxh3` (default) | 16 chars | Non-cryptographic, very fast |
| MD5 | `-a md5` | 32 chars | |
| SHA-256 | `-a sha256` | 64 chars | |
| BLAKE3 | `-a blake3` | 64 chars | Fast cryptographic hash |

## Platform

macOS only. Uses `osascript` for Trash and `xattr` for Finder tags and hash caching.

## Testing

```sh
cargo test              # unit tests
./tests/integration.sh  # integration tests
```

## Options

```
  -a, --algorithm <ALGORITHM>            Hash algorithm [default: xxh3]
  -r, --recursive                        Recurse into directories [default: true]
  -n, --dry-run                          Print hashes without setting attributes
  -v, --verbose                          Print hash and path for each file
      --delete                           Delete duplicates [default: true]
  -k, --keep <KEEP>                      Strategy for which file to keep [default: best]
      --yes                              Skip confirmation, move to Trash
      --hard                             Skip confirmation, permanently delete
      --no-cache                         Force re-hash, ignoring cached values
      --ssim                             Use perceptual similarity for images [default: true]
      --threshold <THRESHOLD>            SSIM threshold for similarity, 0.0-1.0 [default: 0.95]
      --hash-threshold <HASH_THRESHOLD>  Max Hamming distance for hash pre-filter [default: 10]
  -e, --exclude <PATTERN>                Exclude files/dirs matching pattern (repeatable)
  -h, --help                             Print help
```
