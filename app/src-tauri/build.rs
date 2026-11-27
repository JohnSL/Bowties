fn main() {
    let target = std::env::var("TARGET").expect("Cargo did not provide TARGET");
    if !target.ends_with("-windows-msvc") {
        tauri_build::build();
        return;
    }

    let identity = [
        ("type", "win32"),
        ("name", "Microsoft.Windows.Common-Controls"),
        ("version", "6.0.0.0"),
        ("processorArchitecture", "*"),
        ("publicKeyToken", "6595b64144ccf1df"),
        ("language", "*"),
    ];
    let linker_identity = identity
        .iter()
        .map(|(key, value)| {
            assert!(!value.contains(['\'', '\r', '\n']), "invalid linker identity value");
            format!("{key}='{value}'")
        })
        .collect::<Vec<_>>()
        .join(" ");
    let xml_identity = identity
        .iter()
        .map(|(key, value)| {
            let escaped = value.replace('&', "&amp;").replace('<', "&lt;").replace('"', "&quot;");
            format!("        {key}=\"{escaped}\"\n")
        })
        .collect::<String>();
    let manifest = format!(
        "<assembly xmlns=\"urn:schemas-microsoft-com:asm.v1\" manifestVersion=\"1.0\">\n  <dependency>\n    <dependentAssembly>\n      <assemblyIdentity\n{xml_identity}      />\n    </dependentAssembly>\n  </dependency>\n</assembly>\n"
    );

    // Library test harnesses do not receive Tauri's binary-only resource packet.
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTDEPENDENCY:{linker_identity}");
    let windows = tauri_build::WindowsAttributes::new().app_manifest(manifest);
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run Tauri build script");
    // Binaries already have Tauri's manifest; do not generate a duplicate.
    println!("cargo:rustc-link-arg-bins=/MANIFEST:NO");
    println!("cargo:rustc-link-arg-cdylib=/MANIFEST:NO");
}
