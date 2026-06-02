# mvsum

Rename files to their content hash, preserving the file extension and directory.

## Usage

```sh
mvsum [OPTIONS] <PATHS>...
```

## Options

| Flag | Description |
|------|-------------|
| `-a, --algorithm <ALGORITHM>` | Hash algorithm: `xxh3` (default), `md5`, `sha1`, `blake3` |
| `-v, --verbose` | Print renames as they happen |

## Examples

```sh
# Rename using default XXH3
mvsum photo.jpg
# photo.jpg -> a1b2c3d4e5f67890.jpg

# Rename with SHA-1
mvsum -a sha1 *.png

# Verbose output
mvsum -v -a blake3 /path/to/files/*.pdf
```

## Notes

- Skips `.DS_Store` files automatically
- Skips directories (prints a warning)
- Exits with code 1 if any errors occurred
