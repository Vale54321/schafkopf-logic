let
  pkgs = import <nixpkgs> {};
in
pkgs.mkShell {
  buildInputs = with pkgs; [
    cargo
    rustc
    rustPackages.clippy
    pkg-config
    gnupg
    pinentry
    pinentry-curses
  ];
  shellHook = ''
    # Export GPG_TTY so pinentry-curses can attach to the current terminal
    export GPG_TTY=$(tty)
  '';
}
