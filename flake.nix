{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    fenix = {
      url = "github:nix-community/fenix";
    };
    tsunami = {
      url = "github:itepastra/tsunami";
    };
  };

  outputs = { self, fenix, nixpkgs, tsunami, ... }:
    let
      allSystems = [
        "x86_64-linux" # 64-bit Intel/AMD Linux
        "aarch64-linux" # 64-bit ARM Linux
        "x86_64-darwin" # 64-bit Intel macOS
        "aarch64-darwin" # 64-bit ARM macOS
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs allSystems (system: f {
        inherit system;
        inherit tsunami;
        pkgs = import nixpkgs { inherit system; };
        fpkgs = import fenix { inherit system; };
      });
    in
    {
      packages = forAllSystems
        ({ pkgs, fpkgs, ... }:
          let
            toolchain = fpkgs.minimal.toolchain;
            fs = pkgs.lib.fileset;
          in
          rec {
            default = flurry;
            flurry =
              (pkgs.makeRustPlatform { cargo = toolchain; rustc = toolchain; }).buildRustPackage {
                pname = "flurry";
                version = "0.1.0";
                cargoLock.lockFile = ./Cargo.lock;
                src = fs.toSource {
                  root = ./.;
                  fileset = fs.unions [
                    ./Cargo.lock
                    ./Cargo.toml
                    ./src
                  ];
                };
              };
          });
      devShells = forAllSystems
        ({ pkgs, fpkgs, system, ... }:
          let
            ffpkgs = fpkgs.complete;
          in
          {
            default = pkgs.mkShell
              {
                buildInputs = [
                  ffpkgs.cargo
                  ffpkgs.clippy
                  ffpkgs.rust-src
                  ffpkgs.rustc
                  ffpkgs.rustfmt
                  pkgs.wgo
                  tsunami.packages.${system}.tsunami
                ];
              };
          });
      hydraJobs = {
        devShell.x86_64-linux = self.devShells.x86_64-linux.default;
        flurry.x86_64-linux = self.packages.x86_64-linux.flurry;
      };
    };
}

