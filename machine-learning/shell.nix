# Self-contained Python environment; works after moving this directory to its own repo.
{ pkgs ? import <nixpkgs> {} }:
pkgs.mkShell {
  packages = [ (pkgs.python3.withPackages (ps: [ ps.numpy ps.pyarrow ])) ];
  shellHook = ''
    export OPENBLAS_NUM_THREADS=1
  '';
}
