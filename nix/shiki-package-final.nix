{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  cmake,
  perl,
  nasm,
  stdenv,
  openssl,
  oniguruma,
}:

rustPlatform.buildRustPackage (finalAttrs: {
  pname = "shiki";
  version = "0.8.2";

  src = fetchFromGitHub {
    owner = "sazardev";
    repo = "shiki";
    tag = "v${finalAttrs.version}";
    hash = "sha256-A78tDlq6FdWD3KhfWWlTJHhTg4dW0/FKDLQq6Kec1pU=";
  };

  cargoLock.lockFile = "${finalAttrs.src}/Cargo.lock";

  nativeBuildInputs = [
    pkg-config
    cmake
    perl
  ] ++ lib.optionals stdenv.hostPlatform.isx86_64 [ nasm ];

  buildInputs = [
    openssl
    oniguruma
  ];

  env = {
    # git2's Cargo.toml hardcodes the "vendored-openssl" feature (needed for
    # shiki's own cross-platform release binaries, built outside nixpkgs);
    # this overrides openssl-sys's build.rs to link nixpkgs' own openssl
    # instead of compiling OpenSSL from source (same as nixpkgs' gitui
    # package, which hits the identical git2 vendored-openssl situation).
    OPENSSL_NO_VENDOR = 1;
    # syntect's default features pull in onig_sys (the oniguruma C library)
    # for regex-onig; this links nixpkgs' oniguruma via pkg-config instead
    # of compiling its own bundled copy (same as nixpkgs' atac package).
    RUSTONIG_SYSTEM_LIBONIG = true;
  };

  meta = {
    description = "TUI note-taking app with a Yazi-style three-pane layout and modal navigation, notes as plain Markdown + YAML frontmatter, each notebook its own git repo";
    homepage = "https://github.com/sazardev/shiki";
    changelog = "https://github.com/sazardev/shiki/blob/v${finalAttrs.version}/CHANGELOG.md";
    license = lib.licenses.mit;
    maintainers = with lib.maintainers; [ sazardev ];
    mainProgram = "shiki";
  };
})
