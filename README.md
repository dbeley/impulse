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
- 🎶 **Multiple format support** - MP3, FLAC, OGG, WAV, M4A, AAC, Opus, WMA

## Installation

```bash
cargo build --release
```

The binary will be available at `target/release/impulse`.

## Usage

```bash
impulse
```

On first run, a default configuration file will be created at `~/.config/impulse/config.toml`.

## Keybindings

### Global Keys
- `Tab` / `Shift+Tab` - Switch between tabs
- `q` - Quit application
- `?` - Show help message
- `Space` - Play/Pause current track
- `n` - Next track
- `P` - Previous track
- `s` - Stop playback
- `/` - Start search mode
- `:` - Command mode

### Browser Tab
- `j` / `↓` - Move down
- `k` / `↑` - Move up
- `g` - Go to first item
- `G` - Go to last item
- `l` / `Enter` - Enter directory or add file to queue
- `h` - Go to parent directory
- `a` - Add current file to queue
- `A` - Add all files in current directory to queue

### Queue Tab
- `j` / `↓` - Move down
- `k` / `↑` - Move up
- `g` - Go to first track
- `G` - Go to last track
- `Enter` - Jump to selected track
- `d` - Remove selected track
- `c` - Clear queue

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
playlist_dir = "/home/user/.config/impulse/playlists"
volume = 0.5
```

See `config.toml.example` for a complete example configuration.

### Command Mode

Type `:` to enter command mode. Available commands:
- `:quit` or `:q` - Quit the application
- `:save` - Save current configuration
- `:vol <0-100>` or `:volume <0-100>` - Set volume (e.g., `:vol 75`)

## Supported Formats

MP3, FLAC, OGG, WAV, M4A, AAC, Opus, WMA

## Requirements

- Rust 1.70 or later
- ALSA development libraries (Linux)
  - Ubuntu/Debian: `sudo apt-get install libasound2-dev`
  - Fedora: `sudo dnf install alsa-lib-devel`
  - Arch: `sudo pacman -S alsa-lib`

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
