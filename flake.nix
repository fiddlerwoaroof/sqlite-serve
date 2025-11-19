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
        nginx = pkgs.stdenv.mkDerivation {
          name = "nginx-src";
          src = pkgs.nginx.src;
          buildInputs = [
            pkgs.pcre.dev
            pkgs.pkg-config
            pkgs.zlib.dev
          ];
          buildPhase = "";
          installPhase = ''
            mkdir -p "$out"
            cp -R ./ "$out"/
            ls "$out"
          '';
        };
      in {
        packages = {
          nginx-src = nginx;
        };
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          singleStep = false;
          dontStrip = true;
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          buildInputs = [
            nginx
            pkgs.llvmPackages.libclang.lib
            pkgs.pcre.dev
            pkgs.pkg-config
            pkgs.sqlite
            pkgs.zlib.dev
          ];
          preBuild = ''
            export NGINX_BUILD_DIR="${nginx}/objs"
          '';
          postInstall = ''
            mkdir -p "$out"/lib
            cp target/release/libsqlite_serve.dylib "$out"/lib
          '';
        };
        copyLibs = true;
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
