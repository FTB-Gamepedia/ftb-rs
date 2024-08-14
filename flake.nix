{
  description =  "Stuff for FTB wiki";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
	systems.url = "github:nix-systems/default";
    flake-utils = {
      url = "github:numtide/flake-utils";
	  inputs.systems.follows = "systems";
	};
    rust-overlay = {
	  url = "github:oxalica/rust-overlay";
	  inputs.nixpkgs.follows = "nixpkgs";
	};
  };

  outputs = { self, nixpkgs, flake-utils, rust-overlay, ... }:
    flake-utils.lib.eachDefaultSystem (system:
      let
        overlays = [ (import rust-overlay) ];
        pkgs = import nixpkgs { inherit system overlays; };
        rustVersion = pkgs.rust-bin.stable.latest.default;
      in {
        devShell = pkgs.mkShell {
          packages = [ (rustVersion.override { extensions = [ "rust-src" ]; }) ];
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

            nativeBuildInputs = [ pkgs.pkg-config ];
            buildInputs = [ pkgs.openssl ]
			  ++ pkgs.lib.optional pkgs.stdenv.isDarwin [ pkgs.darwin.apple_sdk.frameworks.Foundation pkgs.darwin.apple_sdk.frameworks.Security ]
			  ;
          };
		  default = self.packages.${system}.ftb-rs;
        };

        apps.default = {
          type = "app";
          program = "${self.packages.${system}.ftb-rs}/bin/ftb";
        };
      });
}
