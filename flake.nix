{
  description = "Pass integration for fuzzel.";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
  };

  outputs = {
    self,
    nixpkgs,
    ...
  }: let
    systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
    eachSystem = func:
      nixpkgs.lib.genAttrs systems (system:
        func {
          inherit system;
          pkgs = import nixpkgs {inherit system;};
        });
  in {
    packages = eachSystem ({pkgs, ...}: {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = "fuzzel-pass";
        version = "0.1.0";
        src = ./.;

        cargoLock = {
          lockFile = ./Cargo.lock;
        };
      };
    });

    devShells = eachSystem ({pkgs, ...}: {
      default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
        ];
      };
    });

    formatter = eachSystem ({pkgs, ...}: pkgs.alejandra);
  };
}
