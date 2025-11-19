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
            pkgs.llvmPackages.libclang.lib
            pkgs.pcre.dev
            pkgs.pkg-config
            pkgs.zlib.dev
          ];
          buildPhase = "";
          installPhase = ''
            mkdir -p "$out"
            cp -R objs "$out"/objs
          '';
        };
      in {
        packages.${system} = {
          nginx-src = nginx;
          nginx = pkgs.nginx;
        };
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          singleStep = false;
          dontStrip = true;
          buildInputs = [
            nginx
            pkgs.pcre.dev
            pkgs.pkg-config
            pkgs.sqlite
            pkgs.zlib.dev
          ];
          preBuild = ''
            fw_orig_path="$PWD"
            tar xf "${pkgs.nginx.src}"
            cd "nginx-1.28.0"
            ./configure --with-pcre=${pkgs.pcre.dev} --with-zlib=${pkgs.zlib.dev}
            cd "$fw_orig_path"
            export NGINX_BUILD_DIR="$PWD/nginx-1.28.0/objs"
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
