{
  lib,
  stdenv,
  rustPlatform,
  installShellFiles,
}:

let
  manifest = lib.importTOML ../Cargo.toml;
in
rustPlatform.buildRustPackage (finalAttrs: {
  pname = manifest.package.name;
  version = manifest.package.version;
  src = lib.fileset.toSource {
    root = ../.;
    fileset = lib.fileset.unions [
      ../Cargo.toml
      ../Cargo.lock
      ../src
      ../tests
      ../README.md
      ../LICENSE
    ];
  };
  cargoLock.lockFile = ../Cargo.lock;

  nativeBuildInputs = [ installShellFiles ];
  doCheck = stdenv.buildPlatform.canExecute stdenv.hostPlatform;
  # Text assertions must not depend on the builder having a pseudo-terminal.
  preCheck = ''
    export NO_COLOR=1
  '';
  # Tests exercise HTTP mock servers on loopback.
  __darwinAllowLocalNetworking = true;

  postInstall = lib.optionalString (stdenv.buildPlatform.canExecute stdenv.hostPlatform) ''
    for shell in bash fish zsh; do
      "$out/bin/qnap" completions "$shell" > "qnap.$shell"
    done
    installShellCompletion qnap.{bash,fish,zsh}
  '';

  doInstallCheck = stdenv.buildPlatform.canExecute stdenv.hostPlatform;
  installCheckPhase = ''
    runHook preInstallCheck
    for command in qnap qnap-cli; do
      test "$("$out/bin/$command" --version)" = "qnap ${finalAttrs.version}"
      "$out/bin/$command" --help > /dev/null
    done
    for completion in \
      "$out/share/bash-completion/completions/qnap.bash" \
      "$out/share/fish/vendor_completions.d/qnap.fish" \
      "$out/share/zsh/site-functions/_qnap"; do
      if ! test -s "$completion"; then
        echo "Missing or empty completion file: $completion" >&2
        exit 1
      fi
    done
    runHook postInstallCheck
  '';

  meta = {
    inherit (manifest.package) description homepage;
    license = lib.licenses.mit;
    mainProgram = "qnap";
    platforms = [
      "x86_64-linux"
      "aarch64-linux"
      "aarch64-darwin"
    ];
  };
})
