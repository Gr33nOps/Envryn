// A build script has no user or UI to report failure to beyond aborting the
// build itself, so panicking on an unrecoverable build-environment problem is
// correct here -- the same justification `lib.rs`'s `run()` documents for its
// own `#[allow(clippy::expect_used)]`.
#[allow(clippy::expect_used)]
fn main() {
    tauri_build::build();

    // `tauri_build::build()` embeds a Windows application manifest requesting
    // Common Controls v6 (needed for `SetWindowSubclass`/`RemoveWindowSubclass`/
    // `DefSubclassProc`/`TaskDialogIndirect`, which wry and this crate's own
    // window subclassing in `envryn_core::platform::windows_impl` both use)
    // into `bin` targets only: underneath it, `embed_resource::compile` emits
    // `cargo:rustc-link-arg-bins=...`, and Cargo does not apply that directive
    // to `test` targets. Without a v6-requesting manifest, the OS side-by-side
    // loader resolves `comctl32.dll` imports against the legacy v5 assembly,
    // which lacks those functions entirely -- since imports are bound eagerly
    // at process start, `cargo test`'s own harness binary fails to even reach
    // `main` (`STATUS_ENTRYPOINT_NOT_FOUND`), before any test code runs.
    // Embedding the identical manifest into test binaries specifically (via
    // `compile_for_tests`, which Cargo *does* apply there) fixes that without
    // changing what ships in the real application.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR is set during a build script run");
        let manifest_path = std::path::Path::new(&out_dir).join("test-harness-manifest.xml");
        std::fs::write(
            &manifest_path,
            r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
</assembly>
"#,
        )
        .expect("writes the test-harness manifest");

        let rc_path = std::path::Path::new(&out_dir).join("test-harness-manifest.rc");
        std::fs::write(
            &rc_path,
            format!(
                "1 24 \"{}\"\n",
                manifest_path.display().to_string().replace('\\', "\\\\")
            ),
        )
        .expect("writes the test-harness resource script");

        embed_resource::compile_for_tests(&rc_path, embed_resource::NONE)
            .manifest_required()
            .expect("embeds the Common Controls v6 manifest into test binaries");
    }
}
