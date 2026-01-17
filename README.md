# fuzzel-pass

```
A utility to copy passwords from pass using fuzzel.

Usage: fuzzel-pass [password] [options]...

Positional Arguments:
     [password]
         A password to show directly, skipping the selection.

Options:
     -o,--otp
         Generate and copy/type an OTP code using pass-otp instead of showing password fields.
     -t,--type
         Type the selection instead of copying to the clipboard.
     -h,--help
         Show this help message.
```

As I have recently switched to using [fuzzel](https://codeberg.org/dnkl/fuzzel) instead of [wofi](https://hg.sr.ht/~scoopta/wofi) I needed a
replacement for [wofi-pass](https://github.com/schmidtandreas/wofi-pass). So I sat down and wrote this. Feel free to use
this as you like, make bug reports using issues or open a PR.

# Features

- Select passwords interactively using fuzzel
- Copy or type password fields
- **OTP support** - Generate one-time passwords using [pass-otp](https://github.com/tadfisher/pass-otp)
- Direct password access by name
- Wayland support via wl-clipboard and wtype

# Usage

This assumes that your [pass](https://git.zx2c4.com/password-store) passwords are formatted in the following way:
```
P@ssword123
login: example@example.com
url: https://example.com
```
The password **MUST** be on the first line.

## OTP Support

If you have [pass-otp](https://github.com/tadfisher/pass-otp) installed and configured for a password entry, fuzzel-pass will automatically detect it and offer an "otp" option when selecting fields.

You can also use the `-o` or `--otp` flag to directly generate an OTP code:
```shell
fuzzel-pass -o
```

Or for a specific password:
```shell
fuzzel-pass my-password -o
```

# Dependencies

Required:
- [pass](https://git.zx2c4.com/password-store) - The password manager
- [fuzzel](https://codeberg.org/dnkl/fuzzel) - The application launcher
- [wl-clipboard](https://github.com/bugaevc/wl-clipboard) - For copying to clipboard

Optional:
- [wtype](https://github.com/atx/wtype) - For typing passwords (required if using `-t` flag)
- [pass-otp](https://github.com/tadfisher/pass-otp) - For OTP/2FA support

# Building / Installation

Clone the repository and cd into it:
```shell
git clone https://github.com/d-hain/fuzzel-pass
cd fuzzel-pass
```

## Build from source

```shell
cargo build --release
```

And run using:
```shell
./target/release/fuzzel-pass
```

Or install system-wide:
```shell
cargo install --path .
```

## Build using nix

```shell
nix build
```

And run using:
```shell
./result/bin/fuzzel-pass
```

## Build using nix (without cloning the repo)

```shell
nix build github:d-hain/fuzzel-pass
```

## Install on NixOS using flakes

```nix
# flake.nix
{
    inputs = {
        nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
        fuzzel-pass = {
            url = "github:d-hain/fuzzel-pass";
            # inputs.nixpkgs.follows = "nixpkgs";
        }
    };

    outputs = {
        nixpkgs,
        fuzzel-pass,
        ...
    }: let
        system = "x86_64-linux";
        pkgs = nixpkgs.legacyPackages.${system};
    in {
        nixosConfigurations.default = nixpkgs.lib.nixosSystem {
            system = system;
            specialArgs = {
                fuzzel-pass = fuzzel-pass;
            };

            modules = [
                /path/to/configuration.nix
            ];
        };
    };
}
```

```nix
# configuration.nix

{
    config,
    lib,
    pkgs,
    fuzzel-pass,
    ...
}:
{
    # ...

    users.users.USERNAME = {
        # ...
        packages = with pkgs; [
            (fuzzel-pass.packages.${pkgs.system}.default)
            # ...
        ];
    };

    # ...
}
```

(if enough people use this someday I'll make a PR for adding this to nixpkgs)
