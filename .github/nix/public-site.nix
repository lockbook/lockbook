{ pkgs ? import <nixpkgs> {} }:

let
  version = "0.23.4";
  # The static Linux release also runs on our NixOS CI host.
  zola = pkgs.runCommand "zola-${version}" {
    src = pkgs.fetchurl {
      url = "https://github.com/getzola/zola/releases/download/v${version}/zola-v${version}-x86_64-unknown-linux-musl.tar.gz";
      sha256 = "d99c51302ebbf909a0d83d4319d4d745b56a93dc49c4a69878c0f0dcaa4c8531";
    };
    meta.platforms = [ "x86_64-linux" ];
  } ''
    mkdir -p "$out/bin"
    tar -xzf "$src" -C "$out/bin" zola
    test "$("$out/bin/zola" --version)" = "zola ${version}"
  '';
in
pkgs.mkShell {
  packages = [ zola pkgs.openssl pkgs.google-cloud-sdk ];
}
