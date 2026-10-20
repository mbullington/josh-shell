{
  fetchCrate,
  lib,
  rustPlatform,
  stdenv,
}:

let
  reedline = fetchCrate {
    pname = "reedline";
    version = "0.49.0";
    hash = "sha256-zg+fHKDR1v059/qaJzcxebOmvWJI6NVfV7IF4Bj5JuE=";
  };
in
rustPlatform.buildRustPackage {
  pname = "josh-shell";
  version = (lib.importTOML ./Cargo.toml).workspace.package.version;

  src = ./.;

  cargoHash = "sha256-aChRFdy6PQHThQUJu3pdSyPAx/HZ8ONoKUCaFhG75Tc=";

  # Cargo.toml patches crates-io reedline to target/patch/reedline-0.49.0,
  # which `cargo patch-crate` normally generates (see patches/README.md).
  # Recreate it here from the pinned crate tarball plus the checked-in
  # patch so the sandbox build needs neither patch-crate nor network.
  postPatch = ''
    mkdir -p target/patch/reedline-0.49.0
    cp -R ${reedline}/. target/patch/reedline-0.49.0
    chmod -R u+w target/patch/reedline-0.49.0
    patch -d target/patch/reedline-0.49.0 -p1 < patches/reedline+0.49.0.patch
  '';

  cargoBuildFlags = [ "--workspace" ];

  # REPL integration tests require a controlling tty, which the Nix sandbox
  # does not provide.
  doCheck = false;

  installPhase = ''
    runHook preInstall

    install -D target/${stdenv.hostPlatform.rust.cargoShortTarget}/release/josh $out/bin/josh
    install -D docs/man/josh.1 $out/share/man/man1/josh.1
    cp -r share $out/share/josh

    runHook postInstall
  '';

  passthru.shellPath = "/bin/josh";

  meta = {
    description = "Josh, a small stream-first shell with structured values and a parser-first command language";
    homepage = "https://github.com/mbullington/josh-shell";
    license = lib.licenses.mit;
    mainProgram = "josh";
    platforms = lib.platforms.unix;
  };
}
