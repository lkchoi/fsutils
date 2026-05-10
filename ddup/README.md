# ddup

A macOS CLI tool that hashes files and stores the hash as a Finder tag. Also detects and deletes duplicate files.

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

### Hash and tag files

```sh
# Hash all files in a directory (sets Finder tags)
ddup /path/to/files

# Use a specific algorithm
ddup -a blake3 /path/to/files

# Dry run — print hashes without setting attributes
ddup -n /path/to/files

# Verbose — print hash and path for each file
ddup -v /path/to/files
```

Each file gets two Finder tags: `hash:<value>` and `hashed:<timestamp>`.

### Find duplicates

```sh
# List duplicate files
ddup -d /path/to/files

# Output:
# 9bbb93806422ef35
#   54 KB /path/to/files/a.webp
#   54 KB /path/to/files/b.webp
```

### Delete duplicates

Deleted files are moved to macOS Trash (reversible).

```sh
# Interactive — choose which file to keep per group
ddup -d --delete /path/to/files

# Auto-select with a strategy, confirm once at the end
ddup -d --delete --keep=newest /path/to/files
```

**Keep strategies:**

| Strategy | Keeps |
|---|---|
| `newest` | Most recently modified file |
| `oldest` | Earliest modified file |
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
verbose=true
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
cargo test              # 21 unit tests
./tests/integration.sh  # 24 integration tests
```

## Options

```
  -a, --algorithm <ALGORITHM>  Hash algorithm [default: xxh3]
  -r, --recursive              Recurse into directories [default: true]
  -n, --dry-run                Print hashes without setting attributes
  -v, --verbose                Print hash and path for each file
  -d, --duplicates             Find and print duplicate files
      --delete                 Delete duplicates (move to Trash, requires -d)
  -k, --keep <KEEP>            Strategy for which file to keep when deleting
      --no-cache               Force re-hash, ignoring cached values
  -e, --exclude <PATTERN>      Exclude files/dirs matching pattern (repeatable)
  -h, --help                   Print help
```
