fn main() {
    mac_controls::build();
    // Tauri's own build hooks.
    tauri_build::build();

    // Bake the Swift Concurrency rpath on macOS when linking against
    // `ak820-audio-reactive`'s capture feature. See
    // `docs/HANDOFF.md § 6.9g` for the full reasoning and
    // `crates/ak820-cli/build.rs` for the original logic. The three
    // copies (audio crate's tests, CLI bin, this Tauri bin) drift
    // seldom enough that 30-line duplication beats a shared crate
    // for build-script-only logic.
    swift_rpath::emit();
}

mod mac_controls {
    pub fn build() {
        if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
            return;
        }
        let directory = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
        let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
        let architecture = match std::env::var("CARGO_CFG_TARGET_ARCH").unwrap().as_str() {
            "aarch64" => "arm64",
            "x86_64" => "x86_64",
            other => panic!("Unsupported Mac controls architecture: {other}"),
        };
        let mut command = std::process::Command::new("xcrun");
        command.args([
            "swiftc",
            "-parse-as-library",
            "-O",
            "-emit-library",
            "-static",
            "-module-name",
            "AK820MacControls",
            "-target",
            &format!("{architecture}-apple-macosx11.0"),
        ]);
        command
            .arg("-module-cache-path")
            .arg(output.join("swift-module-cache"));
        for name in [
            "AudioState.swift",
            "ControlModel.swift",
            "Actions.swift",
            "Discovery.swift",
            "Runtime.swift",
            "Mapping.swift",
            "Player.swift",
            "MusicAutomation.swift",
            "Bridge.swift",
        ] {
            let file = directory.join("native/mac-controls").join(name);
            println!("cargo:rerun-if-changed={}", file.display());
            command.arg(file);
        }
        let result = command
            .arg("-o")
            .arg(output.join("libAK820MacControls.a"))
            .output()
            .expect("Xcode Command Line Tools with Swift are required on macOS");
        assert!(
            result.status.success(),
            "Mac controls Swift build failed:\n{}",
            String::from_utf8_lossy(&result.stderr)
        );
        println!("cargo:rustc-link-search=native={}", output.display());
        println!("cargo:rustc-link-lib=static=AK820MacControls");
        for framework in [
            "AppKit",
            "ApplicationServices",
            "Carbon",
            "CoreAudio",
            "Foundation",
        ] {
            println!("cargo:rustc-link-lib=framework={framework}");
        }
        let sdk = std::process::Command::new("xcrun")
            .arg("--show-sdk-path")
            .output()
            .unwrap();
        let sdk = String::from_utf8(sdk.stdout).unwrap();
        println!(
            "cargo:rustc-link-search=native={}/usr/lib/swift",
            sdk.trim()
        );
        let compiler = std::process::Command::new("xcrun")
            .args(["--find", "swiftc"])
            .output()
            .unwrap();
        let compiler = String::from_utf8(compiler.stdout).unwrap();
        if let Some(toolchain) = std::path::Path::new(compiler.trim())
            .parent()
            .and_then(|p| p.parent())
        {
            println!(
                "cargo:rustc-link-search=native={}/lib/swift/macosx",
                toolchain.display()
            );
        }
        println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
    }
}

mod swift_rpath {
    pub fn emit() {
        println!("cargo:rerun-if-env-changed=DEVELOPER_DIR");

        let macos = std::env::var("CARGO_CFG_TARGET_OS")
            .map(|os| os == "macos")
            .unwrap_or(false);
        if !macos {
            return;
        }

        // macOS 13+: resolve @rpath/libswift_Concurrency.dylib via the
        // dyld shared cache (rpath = /usr/lib/swift). Same library Apple
        // frameworks load → single load, no duplicate-class warnings.
        // Older macOS: fall back to a toolchain copy on disk.
        if shared_cache_has_concurrency() {
            println!("cargo:rustc-link-arg=-Wl,-rpath,/usr/lib/swift");
            return;
        }
        let Some(path) = find_swift_concurrency_dir() else {
            println!(
                "cargo:warning=libswift_Concurrency.dylib not found on disk and \
                 this macOS predates the shared-cache copy — the bundled app \
                 will fail to launch when audio-reactive is enabled."
            );
            return;
        };
        println!("cargo:rustc-link-arg=-Wl,-rpath,{path}");
    }

    fn shared_cache_has_concurrency() -> bool {
        let Ok(out) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        else {
            return false;
        };
        let v = String::from_utf8_lossy(&out.stdout);
        let major: u32 = v
            .trim()
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        major >= 13
    }

    fn find_swift_concurrency_dir() -> Option<String> {
        let dev_dir = developer_dir();
        let candidates = [
            "/Library/Developer/CommandLineTools/usr/lib/swift-5.5/macosx".to_string(),
            "/Library/Developer/CommandLineTools/usr/lib/swift/macosx".to_string(),
            format!("{dev_dir}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift-5.5/macosx"),
            format!("{dev_dir}/Toolchains/XcodeDefault.xctoolchain/usr/lib/swift/macosx"),
        ];
        candidates.into_iter().find(|p| {
            std::path::Path::new(p)
                .join("libswift_Concurrency.dylib")
                .exists()
        })
    }

    fn developer_dir() -> String {
        if let Ok(dir) = std::env::var("DEVELOPER_DIR") {
            return dir;
        }
        match std::process::Command::new("xcode-select")
            .arg("-p")
            .output()
        {
            Ok(out) if out.status.success() => {
                String::from_utf8_lossy(&out.stdout).trim().to_string()
            }
            _ => String::new(),
        }
    }
}
