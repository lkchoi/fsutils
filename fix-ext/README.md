# fix-ext

Fix file extensions based on detected MIME type. Uses the `file` command to detect the actual content type and renames files to have the correct extension.

## Usage

```sh
fix-ext [OPTIONS] <PATHS>...
```

## Options

| Flag | Description |
|------|-------------|
| `-n, --dry-run` | Print renames without executing them |
| `-v, --verbose` | Print each rename as it happens |

## Supported Types

| MIME Type | Extension |
|-----------|-----------|
| image/jpeg | .jpg |
| image/png | .png |
| image/gif | .gif |
| image/webp | .webp |
| image/avif | .avif |
| image/heic | .heic |
| image/tiff | .tiff |
| video/mp4 | .mp4 |
| video/quicktime | .mov |
| video/x-matroska | .mkv |
| video/webm | .webm |
| video/x-msvideo | .avi |
| audio/mpeg | .mp3 |
| audio/mp4 | .m4a |
| audio/ogg | .ogg |
| audio/flac | .flac |
| audio/wav | .wav |

## Examples

```sh
# Fix extensions for all files in current directory
fix-ext *

# Preview what would change
fix-ext -n /path/to/files/*

# Verbose mode
fix-ext -v misnamed_file.png another_file.dat
```

## Notes

- Files already having the correct extension are skipped
- For unknown MIME types, the extension is normalized (lowercased, `jpeg` → `jpg`)
- Files without an extension are skipped
- On case-insensitive filesystems (macOS), renames through a temp file to force case changes
- Relies on the macOS/Linux `file` command for MIME detection
