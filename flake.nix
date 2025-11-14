{
  inputs = {
    naersk.url = "github:nix-community/naersk/master";
    nixpkgs.url = "github:NixOS/nixpkgs/nixpkgs-unstable";
    utils.url = "github:numtide/flake-utils";
  };

  outputs = {
    self,
    nixpkgs,
    utils,
    naersk,
  }:
    utils.lib.eachDefaultSystem (
      system: let
        pkgs = import nixpkgs {inherit system;};
        naersk-lib = pkgs.callPackage naersk {};
      in {
        defaultPackage = naersk-lib.buildPackage ./.;
        devShell = with pkgs;
          (mkShell.override {stdenv = pkgs.clangStdenv;}) {
            buildInputs = [
              cargo
              gnumake
              libxcrypt
              nginx
              openssl.dev
              pcre.dev
              pkg-config
              pkgs.llvmPackages.libclang.lib
              rust-analyzer
              rustPackages.clippy
              rustc
              rustfmt
              sqlite
              zlib.dev
            ];

            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
          };
      }
    );
}
