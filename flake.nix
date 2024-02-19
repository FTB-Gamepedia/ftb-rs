{
  description =  "Stuff for FTB wiki";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    rust-overlay.url = "github:oxalica/rust-overlay";
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustVersion = pkgs.rust-bin.stable.latest.default;
      in {
        devShell = pkgs.mkShell {
          buildInputs = [ (rustVersion.override { extensions = [ "rust-src" ]; }) ];
        };

        packages = {
          ftb-rs = pkgs.rustPlatform.buildRustPackage rec {
            pname = "ftb-rs";
            version = "0.1.0";
            src = ./.;

            cargoLock = {
              lockFile = ./Cargo.lock;
              outputHashes = {
                "mediawiki-0.0.1" = "sha256-iekGJXWT4n5ad4nlu27sNfuydL9quvEe4Bw327LgaBE=";
              };
            };

            buildInputs = [ ] ++ pkgs.lib.optional pkgs.stdenv.isDarwin [ pkgs.darwin.apple_sdk.frameworks.Foundation pkgs.darwin.apple_sdk.frameworks.Security ];
          };
        };

        defaultPackage = self.packages.${system}.ftb-rs;

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.ftb-rs}/bin/ftb";
        };
      });
}
