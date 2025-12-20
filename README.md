# impulse

Minimalist music player with minimal dependencies, focusing on speed and a keyboard-centric TUI.

<img width="1375" height="1130" alt="2025-12-20 20-43-42" src="https://github.com/user-attachments/assets/66a0a616-9bd3-46e5-ba4f-4c95ea4858e5" />

## Features

- 🎵 **Folder-based browsing** - No library management, just browse your music folders
- ⌨️  **Vim-inspired keybindings** - Efficient keyboard-centric interface
- 🔍 **Fuzzy search** - Quickly find tracks with fuzzy matching
- 📋 **Queue management** - Build and manage your playback queue
- 💾 **Playlist support** - Create and manage M3U playlists
- 📝 **Load from text files** - Import playlists from "artist - song" format files
- 🎨 **Minimal TUI** - Clean interface with multiple tabs (Browser, Now Playing, Playlists)
- ⚙️  **Configuration file** - Customize settings via TOML config
- 🚀 **Minimal dependencies** - Fast and lightweight
- 🎶 **Multiple format support** - MP3, FLAC, OGG Vorbis, Opus, WAV, M4A, AAC, ALAC
- 🎧 **Last.fm scrobbling** - Optional support for scrobbling to Last.fm
- 🖼️  **Album art display** - Shows embedded and external cover art in the player tab

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
  # Or in your home-manager configuration
  home.packages = [ inputs.impulse.packages.${pkgs.system}.default ];
}
```

## Usage

```bash
impulse

# Load playlist from text file
impulse --load-playlist songs.txt
```

On first run, a default configuration file will be created at `~/.config/impulse/impulse.conf`.
Playlists (including saved queues) default to `~/.local/share/impulse/playlists`.

### Load Playlist from File

Load songs from a text file with "artist - song" format (one per line):

```bash
impulse --load-playlist <file.txt>
```

Example file format:
```
Swans - Blind
Willie Colón - Oh, que sera?
Aphex Twin - #19
```

Songs are matched against your music library and added to the queue. See [LOAD_PLAYLIST.md](LOAD_PLAYLIST.md) for details.

## Keybindings

### Global Keys
- `Tab` / `Shift+Tab` - Switch between tabs
- `q` - Quit application
- `?` - Show help message
- `Space` - Play/Pause current track
- `n` - Next track
- `P` - Previous track
- `>` / `<` - Next/previous track (alternative to `n`/`P`)
- `r` - Toggle random mode (plays queue in random order)
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

### Now Playing Tab
- `+` / `=` - Increase volume
- `-` - Decrease volume
- Displays current track metadata, progress, and album artwork

### Playlists Tab
- `j` / `↓` - Move down
- `k` / `↑` - Move up
- `l` / `Enter` - Add playlist to queue
- `r` - Reload playlists

## Configuration

Edit `~/.config/impulse/impulse.conf`:

```toml
music_dir = "/home/user/Music"
playlist_dir = "/home/user/.local/share/impulse/playlists"
volume = 0.5
```

See `impulse.conf.example` for a complete example configuration.

### Last.fm Scrobbling (Optional)

Impulse supports scrobbling your listening history to Last.fm. To enable this feature:

1. **Obtain Last.fm API credentials**:
   - Create an API account at https://www.last.fm/api/account/create
   - You'll receive an API key and API secret

2. **Configure Impulse**:
   Add the following to your `~/.config/impulse/impulse.conf`:
   ```toml
   [lastfm]
   enabled = true
   api_key = "your_lastfm_api_key"
   api_secret = "your_lastfm_api_secret"
   session_key = "" # keep it empty, it will be populated on first start
   ```

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

## Album Art Support

Impulse displays album artwork in the Now Playing tab. It supports:

- **Embedded cover art**: Extracted from audio file metadata (ID3v2, Vorbis comments, etc.)
- **External cover art**: Automatically finds cover images in the same directory as the audio file
  - Common filenames: `cover.jpg`, `folder.jpg`, `album.jpg`, `front.jpg`, `albumart.jpg` (and `.png` variants)
  - Also searches for any `.jpg`, `.jpeg`, `.png`, `.gif`, or `.webp` files in the directory
- Supports various image formats through the terminal's image protocol (iTerm2, Kitty, etc.)

## Requirements

- Rust 1.85 or later (or Nix with flakes enabled)
- ALSA development libraries (Linux)
  - Ubuntu/Debian: `sudo apt-get install libasound2-dev`
  - Fedora: `sudo dnf install alsa-lib-devel`
  - Arch: `sudo pacman -S alsa-lib`
  - NixOS: Automatically provided in the dev shell

**Note**: This project uses Rust Edition 2024, which requires Rust 1.85+. If you're using an older version, you can temporarily change `edition = "2024"` to `edition = "2021"` in `Cargo.toml`.

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
