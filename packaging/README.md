# Installation Methods

## From Source

```bash
git clone https://github.com/shibuido/claude-pretool-sidecar.git
cd claude-pretool-sidecar
cargo build --release
```

Binaries are produced in `target/release/`:

* `claude-pretool-sidecar` -- main sidecar binary
* `claude-pretool-logger` -- companion FYI logger
* `claude-pretool-analyzer` -- hook payload analyzer

Install to your PATH:

```bash
install -m755 target/release/claude-pretool-{sidecar,logger,analyzer} ~/.local/bin/
```

## Via cargo install

```bash
cargo install claude-pretool-sidecar
```

This installs all three binaries (`claude-pretool-sidecar`, `claude-pretool-logger`, `claude-pretool-analyzer`) to your Cargo bin directory (typically `~/.cargo/bin/`).

## AUR (Arch Linux)

```bash
yay -S claude-pretool-sidecar
```

Or build manually from the PKGBUILD in `packaging/aur/`.

## Homebrew (macOS / Linux)

```bash
# Once the tap is set up:
brew install shibuido/tap/claude-pretool-sidecar

# Or install from the local formula:
brew install --formula packaging/brew/claude-pretool-sidecar.rb
```

## Binary Releases (planned)

Pre-built binaries for Linux (x86_64, aarch64) and macOS (x86_64, aarch64) will be available on the [GitHub Releases](https://github.com/shibuido/claude-pretool-sidecar/releases) page.

Use `packaging/release.sh` to build a release archive locally.
