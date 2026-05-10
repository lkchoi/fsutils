# mvsum

Rename files to their content hash, preserving the file extension and directory.

## Usage

```sh
mvsum [OPTIONS] <PATHS>...
```

## Options

| Flag | Description |
|------|-------------|
| `-a, --algorithm <ALGORITHM>` | Hash algorithm: `md5` (default), `sha1`, `xxh3`, `blake3` |
| `-v, --verbose` | Print renames as they happen |

## Examples

```sh
# Rename using default MD5
mvsum photo.jpg
# photo.jpg -> d41d8cd98f00b204e9800998ecf8427e.jpg

# Rename with SHA-1
mvsum -a sha1 *.png

# Preview renames
mvsum -v -a blake3 /path/to/files/*.pdf
```

## Notes

- Skips `.DS_Store` files automatically
- Skips directories (prints a warning)
- Exits with code 1 if any errors occurred
