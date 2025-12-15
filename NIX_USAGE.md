# Nix Flake Usage

This project provides a Nix flake for easy development and deployment.

## Prerequisites

- Nix with flakes enabled (version 2.4 or later)
- Enable flakes in your Nix configuration:
  ```bash
  # Add to ~/.config/nix/nix.conf or /etc/nix/nix.conf
  experimental-features = nix-command flakes
  ```

## Development

### Enter the development shell

```bash
nix develop
```

This provides:
- Rust toolchain (stable with rust-analyzer)
- ALSA development libraries
- Development tools (cargo-watch, cargo-edit, etc.)
- Pre-commit tools (prek)
- Proper environment variables for building

### Build the application

```bash
nix build
```

The result will be in `./result/bin/impulse`.

### Run directly

```bash
nix run
```

## NixOS Integration

### System-wide installation

Add to your `flake.nix`:

```nix
{
  inputs.impulse.url = "github:dbeley/impulse";

  outputs = { self, nixpkgs, impulse, ... }: {
    nixosConfigurations.yourhostname = nixpkgs.lib.nixosSystem {
      modules = [
        impulse.nixosModules.default
        {
          programs.impulse.enable = true;
        }
      ];
    };
  };
}
```

### Home Manager

You can also use it with Home Manager:

```nix
{ inputs, ... }: {
  home.packages = [
    inputs.impulse.packages.${pkgs.system}.default
  ];
}
```

## CI/CD Integration

### GitHub Actions example

```yaml
- name: Install Nix
  uses: cachix/install-nix-action@v24
  with:
    nix_path: nixpkgs=channel:nixos-unstable
    extra_nix_config: |
      experimental-features = nix-command flakes

- name: Build with Nix
  run: nix build

- name: Run tests
  run: nix develop --command cargo test
```

## Customization

You can override the package in your own flake:

```nix
{
  inputs.impulse.url = "github:dbeley/impulse";

  outputs = { self, nixpkgs, impulse }: {
    packages.x86_64-linux.default = impulse.packages.x86_64-linux.default.override {
      # Your customizations here
    };
  };
}
```

## Troubleshooting

### Audio issues on NixOS

If you encounter audio device errors, ensure ALSA or PulseAudio is properly configured:

```nix
# In your configuration.nix
sound.enable = true;
hardware.pulseaudio.enable = true;
# OR
services.pipewire.enable = true;
```

### Building fails

Try updating the flake inputs:

```bash
nix flake update
nix build
```
