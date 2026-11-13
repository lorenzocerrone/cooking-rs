//! Print how the recipes' ingredient names fit the dictionary
//! (`cooking_rs::report`): names it doesn't know, with near matches, and
//! amounts the grocery list can't add up. Always exits 0: it is a to-do list.
//!
//! ```sh
//! cargo run --bin ingredients
//! ```

use cooking_rs::recipes::{load_catalog, load_recipes};
use cooking_rs::report::ingredient_report;

fn main() {
    print!("{}", ingredient_report(&load_recipes(), &load_catalog()));
}
