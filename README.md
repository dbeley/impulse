# impulse

Minimalist music player with minimal dependencies, focusing on speed and a keyboard-centric TUI.

## Features

- 🎵 **Folder-based browsing** - No library management, just browse your music folders
- ⌨️  **Vim-inspired keybindings** - Efficient keyboard-centric interface
- 🔍 **Fuzzy search** - Quickly find tracks with fuzzy matching
- 📋 **Queue management** - Build and manage your playback queue
- 💾 **Playlist support** - Create and manage M3U playlists
- 🎨 **Minimal TUI** - Clean interface with multiple tabs (Browser, Queue, Player, Playlists)
- ⚙️  **Configuration file** - Customize settings via TOML config
- 🚀 **Minimal dependencies** - Fast and lightweight
- 🎶 **Multiple format support** - MP3, FLAC, OGG Vorbis, Opus, WAV, M4A, AAC, ALAC

## Installation

### Using Cargo

```bash
cargo build --release
```

The binary will be available at `target/release/impulse`.

### Using Nix Flakes

For Nix users, you can build and run directly:

```bash
# Run directly
nix run github:dbeley/impulse

# Build and install
nix build github:dbeley/impulse
./result/bin/impulse

# Enter development shell
nix develop
```

### NixOS Configuration

Add to your NixOS configuration:

```nix
{
  inputs.impulse.url = "github:dbeley/impulse";

  # In your configuration.nix or home.nix:
  programs.impulse.enable = true;
}
```

## Usage

```bash
impulse
```

On first run, a default configuration file will be created at `~/.config/impulse/config.toml`.
Playlists (including saved queues) default to `~/.local/share/impulse/playlists`.

## Keybindings

### Global Keys
- `Tab` / `Shift+Tab` - Switch between tabs
- `q` - Quit application
- `?` - Show help message
- `Space` - Play/Pause current track
- `n` - Next track
- `P` - Previous track
- `>` / `<` - Next/previous track (alternative to `n`/`P`)
- `s` - Stop playback
- `/` - Start search mode
- `:` - Command mode

### Browser Tab
- `j` / `↓` - Move down
- `k` / `↑` - Move up
- `g` - Go to first item
- `G` - Go to last item
- `l` / `→` / `Enter` - Enter directory or add file to queue
- `h` / `←` - Go to parent directory
- `a` - Add current file to queue
- `A` - Add all files in current directory to queue

### Queue Tab
- `j` / `↓` - Move down
- `k` / `↑` - Move up
- `g` - Go to first track
- `G` - Go to last track
- `Enter` - Jump to selected track
- `d` / `Backspace` / `Delete` - Remove selected track
- `K` - Move selected track up
- `J` - Move selected track down
- `S` - Save queue as a playlist in the default folder
- `c` - Clear queue

### Search
- `/` - Enter search mode, type a query, and press Enter to show an overlay of matching audio files (each entry shows the file name and its folder).
- `j` / `↓` / `k` / `↑` - Navigate search results in the overlay.
- `Enter` - Browse to the highlighted file’s folder and select it in the browser.
- `Esc` - Close the search overlay without changing folders.

### Player Tab
- `+` / `=` - Increase volume
- `-` - Decrease volume

### Playlists Tab
- `j` / `↓` - Move down
- `k` / `↑` - Move up
- `l` / `Enter` - Add playlist to queue
- `r` - Reload playlists

## Configuration

Edit `~/.config/impulse/config.toml`:

```toml
music_dir = "/home/user/Music"
playlist_dir = "/home/user/.local/share/impulse/playlists"
volume = 0.5
```

See `config.toml.example` for a complete example configuration.

### Command Mode

Type `:` to enter command mode. Available commands:
- `:quit` or `:q` - Quit the application
- `:save` - Save current configuration
- `:save-queue <name>` - Save the current queue as a playlist (stored in the default playlist folder)
- `:vol <0-100>` or `:volume <0-100>` - Set volume (e.g., `:vol 75`)

## Supported Formats

Impulse supports all major audio formats through the Symphonia audio decoding library:

- **MP3** - MPEG-1/2 Layer 3
- **FLAC** - Free Lossless Audio Codec
- **OGG Vorbis** - Ogg container with Vorbis codec
- **Opus** - Opus codec (via symphonia-adapter-libopus)
- **WAV** - Waveform Audio File Format (PCM, ADPCM)
- **M4A** - MPEG-4 Audio (AAC in MP4 container)
- **AAC** - Advanced Audio Coding
- **ALAC** - Apple Lossless Audio Codec

**Note:** Opus support requires cmake and libopus system libraries. When using the Nix development environment, these dependencies are automatically provided. For non-Nix installations, you'll need to install cmake and libopus-dev on your system.

## Requirements

- Rust 1.70 or later (or Nix with flakes enabled)
- ALSA development libraries (Linux)
  - Ubuntu/Debian: `sudo apt-get install libasound2-dev`
  - Fedora: `sudo dnf install alsa-lib-devel`
  - Arch: `sudo pacman -S alsa-lib`
  - NixOS: Automatically provided in the dev shell

## Development

### Setting up the Development Environment

#### With Nix (Recommended)

```bash
# Enter the development shell
nix develop

# All dependencies and tools are automatically available
cargo build
cargo test
cargo clippy
```

#### Without Nix

Install Rust and the required system dependencies, then:

```bash
cargo build
```

### Pre-commit Hooks

This project uses [prek](https://github.com/pinage404/prek) for managing pre-commit hooks.

```bash
# Install prek (if not using Nix)
cargo install prek

# Run pre-commit checks
prek run

# Run checks on all files
prek run --all

# Install git hooks (optional)
prek install
```

The pre-commit configuration includes:
- `cargo fmt` - Code formatting check
- `cargo clippy` - Linting
- `cargo check` - Compilation check
- File hygiene checks (trailing whitespace, YAML/TOML validation, etc.)

## Architecture

Impulse is built with a modular architecture:

- **Browser**: File system navigation with directory and audio file detection
- **Queue**: Track queue management with add, remove, and navigation
- **Player**: Audio playback using the rodio library
- **Playlist Manager**: M3U playlist loading and management
- **Config**: TOML-based configuration file handling
- **UI**: Terminal UI built with ratatui and crossterm

## License

MIT
