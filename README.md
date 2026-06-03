# fsutils

A collection of macOS file utility CLI tools, written in Rust. Each tool is available as a standalone binary or as a subcommand of the combined `fsutils` binary.

## Tools

| Command | Description |
|---------|-------------|
| [mvsum](mvsum/) | Rename files to their content hash |
| [ddup](ddup/) | Hash files, set Finder tags, find and delete duplicates |
| [dss](dss/) | Write .DS_Store preferences and set Finder comments |
| [ssim](ssim/) | Find near-duplicate images using perceptual hashing and SSIM |
| [fix-ext](fix-ext/) | Fix file extensions based on detected MIME type |
| [towebp](towebp/) | Convert images to WebP, preserving file attributes |

## Installation

```sh
./install.sh
```

This builds all tools in release mode and symlinks the binaries into `~/bin`.

## Usage

Each tool can be invoked standalone:

```sh
mvsum *.jpg
ddup /path/to/files
dss comment "archived" /path/to/folder/
ssim /path/to/images
fix-ext *.png
```

Or via the combined binary:

```sh
fsutils mvsum *.jpg
fsutils ddup /path/to/files
fsutils dss comment "archived" /path/to/folder/
fsutils ssim /path/to/images
fsutils fix-ext *.png
```

## Structure

This is a Cargo workspace. Each tool is its own crate with a `lib.rs` (logic) and `main.rs` (CLI wrapper). The `fsutils` crate depends on all others and exposes them as subcommands.

```
rust/
├── Cargo.toml       # workspace
├── mvsum/
├── ddup/
├── dss/
├── ssim/
├── fix-ext/
├── towebp/
├── fsutils/
└── install.sh
```

## Requirements

- macOS (uses xattr, Finder metadata, `file` command)
- Rust 2024 edition
