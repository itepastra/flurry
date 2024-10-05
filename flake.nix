{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = { self, fenix, nixpkgs, ... }:
    let
      allSystems = [
        "x86_64-linux" # 64-bit Intel/AMD Linux
        "aarch64-linux" # 64-bit ARM Linux
        "x86_64-darwin" # 64-bit Intel macOS
        "aarch64-darwin" # 64-bit ARM macOS
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs allSystems (system: f {
        inherit system;
        pkgs = import nixpkgs { inherit system; };
        fpkgs = import fenix { inherit system; };
      });
    in
    {
      packages = forAllSystems
        ({ system, pkgs, fpkgs }:
          let
            toolchain = fpkgs.minimal.toolchain;
          in
          rec {
            default = flurry;
            flurry =
              (pkgs.makeRustPlatform { cargo = toolchain; rustc = toolchain; }).buildRustPackage {
                pname = "flurry";
                version = "0.1.0";
                cargoLock.lockFile = ./Cargo.lock;
                src = pkgs.lib.cleanSource ./.;
              };
          });
      devShell = forAllSystems ({ system, pkgs, ... }:
        pkgs.mkShell {
          buildInputs = [
            pkgs.rustup
            pkgs.wgo
            pkgs.cargo-flamegraph
            pkgs.cargo-udeps
          ];
        });
    };
}

