# cooking-rs

A small, statically-buildable web app for organizing recipes, planning the coming
days' meals, and getting the grocery list for it. Written in Rust with
[Leptos](https://leptos.dev) (CSR) and built with [Trunk](https://trunkrs.dev).

No backend, no database — recipes are plain JSON files embedded at compile time;
the plan lives in the address bar, so sharing it is sending the link.

## Features

- **Recipes** — every `recipes/*.json` file, with a thumbnail, search, and
  cuisine/tag filters; a quick-view dialog and a full detail page with steps.
- **Planner** — the next two weeks by date, from today; each day is a list of meals,
  added from a searchable picker or a recipe's "Add to week", with per-meal servings and
  a Share button. Past days drop off on their own.
- **Grocery list** — everything the planned days need (all of them, or just today, 3 or
  7 days), converted into each ingredient's
  shopping unit and summed ("1 onion" + "100 g onion" → 250 g; "3 cloves" → 12 g;
  "2 tbsp" → 30 ml; "2 cans"), grouped by aisle. Tick items off as you shop, mark
  things you have at home, add extras that aren't in a recipe, copy as text.
- **The address is the plan** — every change rewrites the plan into the URL, so
  bookmarking, copying the address bar or the Share button all give a link with the
  plan; opening one shows that plan. The browser remembers the last plan (for the
  home-screen icon) and this phone's grocery ticks.
- **Validated data** — `cargo test` checks every recipe file parses, ids match
  file names, ingredients exist, units convert, and images are present.
- **Installable PWA** — `manifest.webmanifest` + service worker (`sw.js`) make the
  app installable and offline-capable.

## Prerequisites

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
```

## Develop

```sh
trunk serve --open   # dev server with live reload
cargo test           # unit tests + data validation (host)
cargo clippy --all-targets -- -D warnings
```

## Build (static site)

```sh
trunk build --release
```

The deployable static site is written to `dist/`.

## Data model

### Ingredient catalog (`ingredients.json`)

One entry per ingredient, keyed by id. `unit` is the **shopping unit** — how you
buy it and how the grocery list sums it — and must be `g`, `ml` or `pcs`. An
optional `count` says how the ingredient is counted in pieces so recipes can use
pieces and the list can still add things up:

```json
{
  "spaghetti": { "name": "Spaghetti", "unit": "g",   "category": "Pasta & Grains" },
  "egg":       { "name": "Egg",       "unit": "pcs", "category": "Dairy & Eggs" },
  "onion":     { "name": "Onion",     "unit": "g",   "category": "Produce",
                 "count": { "per_piece": 150 } },
  "garlic":    { "name": "Garlic",    "unit": "g",   "category": "Produce",
                 "count": { "label": "clove", "per_piece": 4 } },
  "chopped_tomatoes": { "name": "Chopped tomatoes", "unit": "pcs", "category": "Pantry",
                 "count": { "label": "can", "per_piece": 400 } }
}
```

`per_piece` is how much of the shopping unit one piece is. For a `pcs`-shopped
ingredient it is the mass of one piece in grams, so "400 g chopped tomatoes"
becomes one can.

### Recipes (`recipes/*.json`)

Recipes reference catalog ingredients **by key**. `unit` may be any of
`g kg ml l tsp tbsp cup pcs` or the ingredient's piece label; omit it to use the
shopping unit. Omit `amount` for "to taste".

```json
{
  "id": "lentil_dal",
  "title": "Lentil Dal",
  "author": "Lorenzo",
  "servings": 4,
  "cuisine": "Indian",
  "tags": ["dinner", "vegetarian"],
  "prep_time": 10,
  "cook_time": 30,
  "image": "images/lentil_dal.svg",
  "ingredients": [
    { "ingredient": "lentils", "amount": 300 },
    { "ingredient": "onion", "amount": 1, "unit": "pcs", "note": "finely chopped" },
    { "ingredient": "garlic", "amount": 3, "unit": "cloves" },
    { "ingredient": "olive_oil", "amount": 1, "unit": "tbsp" },
    { "ingredient": "chives", "amount": 5, "optional": true },
    { "ingredient": "salt" }
  ],
  "steps": ["…"]
}
```

JSON Schemas for both files live in `schema/` and are wired up for VS Code in
`.vscode/settings.json` (autocomplete and inline errors while editing).

### Adding a recipe

Write a plain-text draft:

```text
Lentil Dal
author: Lorenzo
servings: 4
cuisine: Indian
tags: dinner, vegetarian
prep: 10
cook: 30

300 g lentils
1 onion, finely chopped
3 cloves garlic
1/2 can coconut milk
chives (optional)
salt

Steps
Soften the onion and garlic in olive oil.
Simmer everything for 25 minutes.
```

then run the scaffolder and the validation suite:

```sh
cargo run --bin scaffold -- draft.txt     # --dry-run to print, --force to overwrite
cargo test
```

A leading number is the amount. The next word is the unit if it is one
(`g kg ml l tsp tbsp cup pcs`) or the ingredient's piece label (`cloves`, `can`).
A bare number counts pieces ("2 onion" is two onions), text after a comma is a
note, and a line without an amount is "to taste". Every recipe needs an
`author:` (the bundled ones say "Claude"), and the recipe list gets "by …"
filter chips as soon as there are two authors. The scaffolder writes
`recipes/<id>.json` and a placeholder `images/<id>.svg`. If anything is wrong
(an unknown ingredient, a unit that doesn't convert), it lists every problem by
line, prints a catalog entry to paste into `ingredients.json` for each unknown
ingredient, and writes nothing. New catalog entries need an aisle from
`shopping::CATEGORY_ORDER`.

## Deploy (GitHub Pages)

Pushing to `main` runs `.github/workflows/deploy.yml`, which builds with the
correct base path and publishes `dist/` to GitHub Pages.

One-time setup: in the repo, **Settings → Pages → Build and deployment → Source:
GitHub Actions** (without this the deploy job fails).

The base path assumes a project page at `https://<user>.github.io/cooking-rs/`.
If you rename the repo or use a user/root or custom-domain page, update **both**
`--public-url` and `APP_BASE_PATH` in the workflow (root page → `/` and `""`).

## Project layout

```
ingredients.json    ingredient catalog (embedded at build time)
recipes/            recipe data (embedded at build time)
images/             per-recipe images (copied into dist/ via index.html)
schema/             JSON Schemas for the two data files (editor support)
build.rs            re-runs the build when recipes/ or the catalog change
src/
  lib.rs            the crate's modules, shared by both binaries
  main.rs           wasm entry point: mounts the app
  bin/scaffold.rs   host-only recipe scaffolder (cargo run --bin scaffold)
  scaffold.rs       draft text -> validated Recipe, JSON + placeholder SVG (pure)
  app.rs            router + nav shell; owns the shared plan + grocery state
  models.rs         Recipe / RecipeIngredient / CatalogIngredient / MealPlan
  quantity.rs       units, dimensions, piece specs, conversions (pure)
  recipes.rs        data loading boundary (embed today, fetch-ready)
  validate.rs       data validation tests (cargo test)
  filter.rs         recipe search + cuisine/tag filtering (pure)
  shopping.rs       meal plan -> converted, summed, aisle-grouped grocery lines (pure)
  format.rs         display strings: quantities, rounding, text export (pure)
  state.rs          meal plan <-> URL hash (the address is the plan)
  storage.rs        last plan + grocery ticks <-> localStorage
  pwa.rs            service-worker registration
  components/       nav, modal, recipe overview, recipe picker, add-to-planner, icons
  pages/            recipes, recipe detail, planner, grocery
```
