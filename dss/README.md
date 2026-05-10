# dss

Finder metadata utilities: write .DS_Store files to configure Finder view settings, and set Finder comments on files.

## Usage

```sh
dss <COMMAND>
```

## Commands

### write

Write .DS_Store files to set Finder icon view preferences recursively.

```sh
dss write [OPTIONS] [PATH]
```

| Flag | Description |
|------|-------------|
| `--icon-size <SIZE>` | Icon size in pixels (default: 128) |
| `--arrange-by <VALUE>` | Arrange by: `none`, `name`, `dateModified`, `dateCreated`, `dateLastOpened`, `dateAdded`, `size`, `kind`, `label`, `tag` (default: `size`) |
| `--show-preview` | Show icon preview / thumbnails |
| `--dry-run` | Print directories without writing |
| `--clean` | Delete all .DS_Store files instead of writing |

### comment

Set Finder comments on files via the `com.apple.metadata:kMDItemFinderComment` xattr.

```sh
dss comment [OPTIONS] <COMMENT> <PATHS>...
```

| Flag | Description |
|------|-------------|
| `-r, --recursive` | Recurse into directories (default: true) |
| `-e, --exclude <PATTERN>` | Exclude files matching glob pattern (repeatable) |
| `-n, --dry-run` | Print files without setting comment |

## Examples

```sh
# Set icon view to 256px icons sorted by name
dss write --icon-size 256 --arrange-by name /path/to/folder

# Remove all .DS_Store files
dss write --clean /path/to/project

# Set Finder comment on PDFs
dss comment "archived 2024" /path/to/documents/*.pdf

# Preview which files would be commented
dss comment "draft" /path/to/folder --dry-run
```
