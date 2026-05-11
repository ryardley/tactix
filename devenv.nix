{
  pkgs,
  inputs,
  ...
}: let
  pkgs-unstable = import inputs.nixpkgs-unstable {
    system = pkgs.stdenv.system;
  };
in {
  packages = with pkgs; [
    pkgs-unstable.opencode
  ];

  languages.rust = {
    enable = true;
    channel = "stable";
    components = [
      "rustc"
      "cargo"
      "clippy"
      "rustfmt"
      "rust-analyzer"
      "rust-src"
    ];
  };
}
