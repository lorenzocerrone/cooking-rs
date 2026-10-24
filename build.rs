//! Recipes and the catalog are embedded with `include_dir!`/`include_str!`.
//! Cargo tracks edits to files it has already embedded, but not files *added to
//! or removed from* `recipes/`; without this, a new recipe (valid or not) is
//! invisible to `cargo test` and `trunk serve` until an unrelated rebuild.
fn main() {
    println!("cargo:rerun-if-changed=recipes");
    println!("cargo:rerun-if-changed=ingredients.json");
    println!("cargo:rerun-if-changed=build.rs");
}
