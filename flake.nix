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
        pkgs = import nixpkgs {
          inherit system;
        };
        cc-pkgs = import nixpkgs {
          inherit system;
          config.allowUnfreePredicate = pkg:
            builtins.elem (pkgs.lib.getName pkg) [
              "claude-code"
            ];
        };
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

        # Platform-specific configuration
        libExtension = pkgs.stdenv.hostPlatform.extensions.sharedLibrary;
        bindgenFlags =
          if pkgs.stdenv.isLinux
          then "-isystem ${pkgs.glibc.dev}/include -isystem ${pkgs.pcre.dev}/include -L${pkgs.glibc}/lib"
          else "-isystem ${pkgs.pcre.dev}/include";
        platformInputs =
          if pkgs.stdenv.isLinux
          then [
            pkgs.glibc
            pkgs.glibc.dev
          ]
          else [];
      in {
        packages = {
          nginx-src = nginx;
        };
        defaultPackage = naersk-lib.buildPackage {
          src = ./.;
          singleStep = false;
          dontStrip = true;
          LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
          BINDGEN_EXTRA_CLANG_ARGS = bindgenFlags;
          buildInputs =
            [
              nginx
              pkgs.llvmPackages.libclang.lib
              pkgs.pcre.dev
              pkgs.pkg-config
              pkgs.sqlite
              pkgs.zlib.dev
            ]
            ++ platformInputs;
          preBuild = ''
            export NGINX_BUILD_DIR="${nginx}/objs"
          '';
          postInstall = ''
            mkdir -p "$out"/lib
            cp target/release/libsqlite_serve${libExtension} "$out"/lib
          '';
        };
        copyLibs = true;
        devShell = with pkgs;
          (mkShell.override {stdenv = pkgs.clangStdenv;}) {
            buildInputs =
              [
                cargo
                cc-pkgs.claude-code
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
                nginx
              ]
              ++ platformInputs;

            NGINX_BUILD_DIR = "${nginx}/objs";
            LIBCLANG_PATH = "${pkgs.llvmPackages.libclang.lib}/lib";
            BINDGEN_EXTRA_CLANG_ARGS = bindgenFlags;
            RUST_SRC_PATH = rustPlatform.rustLibSrc;
          };
      }
    );
}
