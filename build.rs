fn main() {
    // Embed the application icon into the Windows executable so it shows up in
    // Explorer, the taskbar, and the title bar of the release build.
    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        if let Err(e) = res.compile() {
            // Don't fail the build if the resource compiler is unavailable;
            // just warn (the exe will build without an embedded icon).
            println!("cargo:warning=failed to embed icon: {e}");
        }
    }
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
}
