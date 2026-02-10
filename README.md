# fuzzel-pass

```
A utility to copy passwords from pass using fuzzel.

Usage: {} [password] [options]...

Positional Arguments:
     [password]
         A password to show directly, skipping the selection.

Options:
     -t,--type
         Type the selection instead of copying to the clipboard.
     -h,--help
         Show this help message.
```

As I have recently switched to using [fuzzel](https://codeberg.org/dnkl/fuzzel) instead of [wofi](https://hg.sr.ht/~scoopta/wofi) i needed a
replacement for [wofi-pass](https://github.com/schmidtandreas/wofi-pass). So I sat down and wrote this. Feel free to use
this as you like, make bug reports using issues or open a PR.

# Important!

This assumes that your [pass](https://git.zx2c4.com/password-store) passwords are formatted in the following way:
```
P@ssword123
login: example@example.com
url: https://example.com
```
The password **MUST** be on the first line.

## Multiline Fields

Multiline fields are also supported. The syntax is the following:
```
Field Key:
SOME_UNIQUE_STRING
value
more value
value value value
SOME_UNIQUE_STRING
```
- The field key must have nothing but emptyness after the colon!
- Here `SOME_UNIQUE_STRING` is the marker for the beginning and end of the
  multiline value. It MUST NOT occur in the value itself.

Full example for a password file:
```
password123
field1: github.com
field with space: example value
Private Key:
   PRIVATEKEY
-----BEGIN OPENSSH PRIVATE KEY-----
some private key string
asouu("("(=dsab&(odnsaon6t2
"/&%"("%"(/&"KN")"N§!N"!OINE"
S(/"Bpon)(/")(":89n""=?B&
test
-----END OPENSSH PRIVATE KEY-----
PRIVATEKEY
another field: http://example.com
```
(the password line CAN be empty, but it MUST exist)

# Building / Installation

Clone the repository and cd into it:
```shell
git clone https://github.com/d-hain/fuzzel-pass
```
```shell
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
            specialArgs = { inherit fuzzel-pass; };

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
