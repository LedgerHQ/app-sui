{ localSystem ? { system = builtins.currentSystem; }
}:

rec {
  alamgu = import ./dep/alamgu { inherit localSystem; };

  inherit (alamgu) lib pkgs crate2nix alamguLib;

  appName = "sui";

  app-nix = alamgu.crate2nix-tools.generatedCargoNix {
    name = "${appName}-nix";
    src = builtins.filterSource (p: _: p != toString "./rust-app/target") ./rust-app;
    additionalCrateHashes = builtins.fromJSON (builtins.readFile ./crate-hashes.json);
  };

  makeLinkerScript = { pkgs, sdkSrc }:
    pkgs.stdenvNoCC.mkDerivation {
      name = "alamgu-linker-wrapper";
      dontUnpack = true;
      dontBuild = true;
      installPhase = ''
        mkdir -p "$out/bin"
        if [ -d "${sdkSrc}" ]; then
            cp "${sdkSrc}/ledger_device_sdk/link_wrap.sh" "$out/bin"
        else
            cd "$out/bin"; tar xf "${sdkSrc}" --wildcards "*link_wrap.sh" --transform='s:.*/::'
        fi
        substituteInPlace $out/bin/link_wrap.sh \
          --replace 'llvm-objcopy' '$OBJCOPY' \
          --replace 'llvm-nm' '$NM'
        chmod +x "$out/bin/link_wrap.sh"
      '';
    };

  makeApp = { rootFeatures ? [ "default" ], release ? true, device }:
    let collection = alamgu.perDevice.${device};
        ledger-secure-sdk-path = import ./dep/ledger-secure-sdk-${device}/thunk.nix;
        bindings = (alamgu.thunkSource ./dep/ledger_secure_sdk_sys-bindings) + "/${device}/bindings.rs";
    in import app-nix {
      inherit rootFeatures release;
      pkgs = collection.ledgerPkgs;
      buildRustCrateForPkgs = alamguLib.combineWrappers [
        # The callPackage of `buildRustPackage` overridden with various
        # modified arguemnts.
        (pkgs: (collection.buildRustCrateForPkgsLedger pkgs).override {
          defaultCrateOverrides = pkgs.defaultCrateOverrides // {
            ledger_device_sdk = attrs: {
              passthru = (attrs.passthru or {}) // {
                link_wrap = makeLinkerScript {
                  pkgs = pkgs.buildPackages;
                  sdkSrc = attrs.src;
                };
              };
            };
            ${appName} = attrs: let
              sdk = lib.findFirst (p: lib.hasPrefix "rust_ledger_device_sdk" p.name) (builtins.throw "no sdk!") attrs.dependencies;
            in {
              preHook = collection.gccLibsPreHook;
              extraRustcOpts = attrs.extraRustcOpts or [] ++ [
                "-C" "linker=${sdk.link_wrap}/bin/link_wrap.sh"
                "-C" "link-arg=-T${sdk.lib}/lib/ledger_device_sdk.out/link.ld"
                "-C" "link-arg=-T${sdk.lib}/lib/ledger_device_sdk.out/${device}_layout.ld"
              ];
              passthru = (attrs.passthru or {}) // { inherit sdk; };
            };

            # HACK: Dont do bindgen in ledger_secure_sdk_sys, patch the build.rs and inject pre-generated bindings.rs
            ledger_secure_sdk_sys = attrs: {
              patches = [ ./disable-generate-bindings.patch ];
              postUnpack = ''
                substituteInPlace $sourceRoot/src/lib.rs \
                  --replace "concat!(env!(\"OUT_DIR\"), \"/bindings.rs\")" "\"./bindings.rs\""
                cp ${bindings} $sourceRoot/src/bindings.rs
              '';
              preConfigure = ''
                export LEDGER_SDK_PATH="${ledger-secure-sdk-path}"
              '';
            };

            # HACK: Dont build bindgen, and other packages
            rustix = attrs: {
              buildPhase = ''
                touch "$out"
              '';
              installPhase = ''
                touch "$out"
              '';
            };
            which = attrs: {
              buildPhase = ''
                touch "$out"
              '';
              installPhase = ''
                touch "$out"
              '';
            };
            bindgen = attrs: {
              buildPhase = ''
                touch "$out"
              '';
              installPhase = ''
                touch "$out"
              '';
            };
          };
        })

        # Default Alamgu wrapper
        alamguLib.extraArgsForAllCrates

        # Another wrapper specific to this app, but applying to all packages
        (pkgs: args: args // lib.optionalAttrs (alamguLib.platformIsBolos pkgs.stdenv.hostPlatform) {
          dependencies = map (d: d // { stdlib = true; }) [
            collection.ledgerCore
            collection.ledgerCompilerBuiltins
          ] ++ args.dependencies;
        })
      ];
  };

  makeArchiveSource = { appExe, device }:
  let collection = alamgu.perDevice.${device};
  in collection.ledgerPkgs.runCommandCC "${appName}-${device}-tar-src" {
    nativeBuildInputs = [
      alamgu.cargo-ledger
      alamgu.ledgerctl
      alamgu.ledgerRustPlatform.rust.cargo
    ];
    strictDeps = true;
  } (alamgu.cargoLedgerPreHook + ''

    cp ${./rust-app/Cargo.toml} ./Cargo.toml
    # So cargo knows it's a binary
    mkdir src
    touch src/main.rs

    cargo ledger --use-prebuilt ${appExe} --hex-next-to-json build ${device}

    dest=$out/${appName}-${device}
    mkdir -p $dest/dep

    # Copy Alamgu build infra thunk
    cp -r ${./dep/alamgu} $dest/dep/alamgu

    # Create a file to indicate what device this is for
    echo ${device} > $dest/device
    cp app_${device}.json $dest/app.json
    cp app.hex $dest
    cp ${./tarball-default.nix} $dest/default.nix
    cp ${./tarball-shell.nix} $dest/shell.nix
    cp ${./rust-app/sui.gif} $dest/sui.gif
    cp ${./rust-app/sui-small.gif} $dest/sui-small.gif
  '');

  inherit
    (import ./ts-tests { inherit pkgs; })
    testModules
    testScript
    testPackage
    ;

  apiPort = 5005;

  # Tests don't yet run on Darwin
  runTests = { appExe, device, variant ? "", speculosCmd }:
  if pkgs.stdenv.hostPlatform.isDarwin
  then null
  else
  pkgs.runCommandNoCC "run-tests-${device}${variant}" {
    nativeBuildInputs = [
      pkgs.wget alamgu.speculos.speculos testScript
    ];
    strictDeps = true;
  } ''
    mkdir $out
    (
    set +e # Dont exit on error, do the cleanup/kill of background processes
    ${toString speculosCmd} ${appExe} --display headless &
    SPECULOS=$!

    until wget -O/dev/null -o/dev/null http://localhost:${toString apiPort}; do sleep 0.1; done;

    ${testScript}/bin/mocha-wrapper
    rv=$?
    kill -9 $SPECULOS
    exit $rv) | tee $out/short |& tee $out/full &
    TESTS=$!
    (sleep 3m; kill $TESTS) &
    TESTKILLER=$!
    wait $TESTS
    rv=$?
    kill $TESTKILLER
    cat $out/short
    exit $rv
  '';

  makeStackCheck = { rootCrate, device, memLimit, variant ? "" }:
  pkgs.runCommandNoCC "stack-check-${device}${variant}" {
    nativeBuildInputs = [ alamgu.stack-sizes ];
    strictDeps = true;
  } ''
    stack-sizes --mem-limit=${toString memLimit} ${rootCrate}/bin/${appName} ${rootCrate}/bin/*.o | tee $out
  '';

  appForDevice = device: rec {
    app = makeApp { inherit device; };
    app-with-logging = makeApp {
      inherit device;
      release = false;
      rootFeatures = [ "default" "speculos" "extra_debug" ];
    };

    memLimit = {
      nanos = 4500;
      nanosplus = 400000;
      nanox = 400000;
    }.${device} or (throw "Unknown target device: `${device}'");

    stack-check = makeStackCheck { inherit memLimit rootCrate device; };
    stack-check-with-logging = makeStackCheck {
      inherit memLimit device;
      rootCrate = rootCrate-with-logging;
      variant = "-with-logging";
    };

    rootCrate = app.rootCrate.build;
    rootCrate-with-logging = app-with-logging.rootCrate.build;

    appExe = rootCrate + "/bin/" + appName;

    rustShell = alamgu.perDevice.${device}.rustShell.overrideAttrs (old: {
      nativeBuildInputs = old.nativeBuildInputs ++ [
        pkgs.yarn
        pkgs.wget
        (makeLinkerScript {
          inherit pkgs;
          sdkSrc = alamgu.thunkSource ./dep/ledger-nanos-sdk;
        })
      ];
      shellHook = old.shellHook + ''
        export TARGET_JSON="${alamgu.thunkSource ./dep/ledger-nanos-sdk}/ledger_device_sdk/${device}.json"
      '';
    });

    archiveSource = makeArchiveSource { inherit appExe device; };

    tarball = pkgs.runCommandNoCC "${appName}-${device}.tar.gz" {} ''
      dir="${appName}-${device}"
      cp -r "${archiveSource}/$dir" ./
      chmod -R ugo+w "$dir"
      tar -czvhf $out -C . "${appName}-${device}"
    '';

    zip = pkgs.runCommandNoCC "${appName}-${device}.zip" {
      nativeBuildInputs = [ pkgs.zip ];
    } ''
      dir="${appName}-${device}"
      cp -r "${archiveSource}/$dir" ./
      chmod -R ugo+w "$dir"
      zip -r $out "${appName}-${device}"
    '';

    loadApp = pkgs.writeScriptBin "load-app" ''
      #!/usr/bin/env bash
      cd ${archiveSource}/${appName}-${device}
      ${alamgu.ledgerctl}/bin/ledgerctl install -f ${archiveSource}/${appName}-${device}/app.json
    '';

    tarballShell = import (archiveSource + "/${appName}-${device}/shell.nix");

    speculosDeviceFlags = {
      nanos = [ "-m" "nanos" ];
      nanosplus = [ "-m" "nanosp" ];
      nanox = [ "-m" "nanox" ];
    }.${device} or (throw "Unknown target device: `${device}'");

    speculosCmd = [
      "speculos"
      "--api-port" (toString apiPort)
    ] ++ speculosDeviceFlags;

    test = runTests { inherit appExe speculosCmd device; };
    test-with-logging = runTests {
      inherit speculosCmd device;
      appExe = rootCrate-with-logging + "/bin/" + appName;
      variant = "-with-logging";
    };

    appShell = pkgs.mkShell {
      packages = [ alamgu.ledgerctl loadApp alamgu.generic-cli pkgs.jq ];
    };
  };

  nanos = appForDevice "nanos";
  nanosplus = appForDevice "nanosplus";
  nanox = appForDevice "nanox";
  flex = appForDevice "flex";
  stax = appForDevice "stax";

  cargoFmtCheck = pkgs.stdenv.mkDerivation {
    pname = "cargo-fmt-${appName}";
    inherit (nanos.rootCrate) version src;
    nativeBuildInputs = [
      pkgs.alamguRustPackages.cargo
      pkgs.alamguRustPackages.rustfmt
    ];
    buildPhase = ''
      cargo fmt --all --check
    '';
    installPhase = ''
      touch "$out"
    '';
  };

  inherit (pkgs.nodePackages) node2nix;

  nixpkgs-unstable = import ./dep/nixpkgs-unstable {
    inherit localSystem;
  };

  sui-node-shell = nixpkgs-unstable.mkShell {
    strictDeps = true;
    nativeBuildInputs = with nixpkgs-unstable.buildPackages; [
      rustc
      cargo
      rustPlatform.bindgenHook
      pkg-config
    ];
    buildInputs = with pkgs; [
      libclang openssl postgresql.lib rocksdb
    ];
  };

  sui-wallet-shell = nixpkgs-unstable.mkShell {
    strictDeps = true;
    nativeBuildInputs = with nixpkgs-unstable.buildPackages; lib.optional stdenv.isLinux [
      # TODO make avaiable everywhere or skip whole thing on macOS
      turbo
    ] ++ [
      nodejs-14_x nodePackages.pnpm
    ];
  };
}
