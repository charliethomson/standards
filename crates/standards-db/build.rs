// Rebuild when the served content or the release commit changes.
//
// rust-embed's `include_bytes!` only tracks files that existed at the last build, so a new
// doc, skill asset or template would otherwise not be embedded until an unrelated change.
// A directory passed to `rerun-if-changed` is scanned recursively by cargo.

fn main() {
    println!("cargo:rerun-if-env-changed=RELEASE_COMMIT");
    for path in ["docs", "skills", "templates", "CHANGELOG.md"] {
        println!("cargo:rerun-if-changed=../../{path}");
    }
}
