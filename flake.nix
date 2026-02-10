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
    systems = ["x86_64-linux" "aarch64-linux"];
    eachSystem = func:
      nixpkgs.lib.genAttrs systems (system:
        func {
          inherit system;
          pkgs = import nixpkgs {inherit system;};
        });
  in {
    packages = eachSystem ({
      pkgs,
      system,
    }: {
      default = pkgs.rustPlatform.buildRustPackage {
        pname = "fuzzel-pass";
        version = "0.3.0";
        src = ./.;

        cargoLock = {
          lockFile = ./Cargo.lock;
        };

        nativeBuildInputs = with pkgs; [
          makeWrapper
        ];

        buildInputs = with pkgs; [
          wl-clipboard
          wtype
        ];

        postInstall = ''
          wrapProgram "$out/bin/fuzzel-pass" \
            --prefix PATH : "${with pkgs; lib.makeBinPath [wl-clipboard wtype]}"
        '';
      };
    });

    devShells = eachSystem ({pkgs, ...}: {
      default = pkgs.mkShell {
        packages = with pkgs; [
          cargo
          wl-clipboard
          wtype
        ];
      };
    });

    formatter = eachSystem ({pkgs, ...}: pkgs.alejandra);
  };
}
