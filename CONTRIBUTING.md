# Contributing to Impulse

Thank you for your interest in contributing to Impulse!

## Getting Started

1. Fork the repository
2. Clone your fork: `git clone https://github.com/YOUR_USERNAME/impulse.git`
3. Create a new branch: `git checkout -b feature/your-feature-name`
4. Make your changes
5. Test your changes: `cargo test`
6. Build the project: `cargo build --release`
7. Commit your changes: `git commit -am 'Add some feature'`
8. Push to the branch: `git push origin feature/your-feature-name`
9. Submit a pull request

## Development Guidelines

### Code Style

- Follow Rust standard conventions
- Use `cargo fmt` to format your code
- Use `cargo clippy` to catch common mistakes
- Add documentation comments for public APIs

### Testing

- Add tests for new features
- Ensure all tests pass: `cargo test`
- Test the application manually before submitting

### Commit Messages

- Use clear and descriptive commit messages
- Start with a verb in present tense (e.g., "Add", "Fix", "Update")
- Keep the first line under 72 characters

## Project Structure

```
impulse/
├── src/
│   ├── main.rs         # Application entry point
│   ├── browser.rs      # File browser implementation
│   ├── config.rs       # Configuration handling
│   ├── player.rs       # Audio playback
│   ├── playlist.rs     # Playlist management
│   ├── queue.rs        # Queue management
│   └── ui.rs          # Terminal UI implementation
├── Cargo.toml         # Dependencies and metadata
└── README.md          # Project documentation
```

## Feature Requests and Bug Reports

- Use GitHub Issues to report bugs or request features
- Provide as much detail as possible
- Include steps to reproduce for bugs
- Check if the issue already exists before creating a new one

## Areas for Contribution

Some ideas for contributions:

- Add support for more audio formats
- Implement seeking within tracks
- Add support for cover art display
- Improve search functionality (incremental search, regex)
- Add keyboard shortcuts customization
- Add shuffle and repeat modes
- Implement audio visualizations
- Add plugin system for extensibility
- Improve error handling and user feedback

## Questions?

Feel free to open an issue for questions or discussions.
