//! Runs the compile-fail fixture in `tests/compile-fail/` and asserts it
//! does *not* compile -- the mechanical proof behind AI-INV-003 and
//! `docs/AI_SECURITY.md` section 2's claim that "the model received raw
//! vault data" is a compile error. If a future change makes
//! `SanitizedPrompt`'s field `pub` (or adds any other way to construct one
//! outside `ai::gateway`), the fixture starts compiling and this test fails.
//!
//! `tests/compile-fail/sanitized_prompt_construction.stderr` pins the exact
//! rustc diagnostic. If a Rust toolchain upgrade changes that wording, this
//! test starts failing on a *message* mismatch rather than a security
//! regression -- regenerate the `.stderr` file with `TRYBUILD=overwrite
//! cargo test -p envryn-core --test sanitized_prompt_encapsulation` and
//! confirm the failure is still `E0423` (private field) before accepting
//! the new file, not any other error code.

#[test]
fn sanitized_prompt_cannot_be_constructed_outside_the_gateway_module() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/compile-fail/sanitized_prompt_construction.rs");
}
