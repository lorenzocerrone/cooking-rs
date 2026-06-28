# cooking-rs

A small, statically-buildable web app for visualizing cooking recipes and
building a shareable weekly meal plan. Written in Rust with
[Leptos](https://leptos.dev) (CSR) and built with [Trunk](https://trunkrs.dev).

No backend, no database — recipes are plain JSON files embedded at compile time.

## Features

- **Recipes page** — lists all recipes from `recipes/*.json`, each with a thumbnail
  image, and supports **search plus cuisine/tag filtering**.
- **Per-recipe images** — bundled `images/<id>.svg` assets, resolved through a
  base-path-aware helper so they work under a subpath deploy.
- **Nutrition** — per-recipe and per-meal macro summaries, computed from the
  ingredient catalog when a recipe doesn't specify its own macros.
- **Meal planner** — a 7-day × 3-slot (breakfast/lunch/dinner) grid; export the
  selection as a plain-text shopping list.
- **Shareable state** — the meal plan is encoded into the URL hash fragment, so
  copying the URL shares the plan.
- **Installable PWA** — `manifest.webmanifest` + service worker (`sw.js`) make the
  app installable and offline-capable.

## Prerequisites

```sh
rustup target add wasm32-unknown-unknown
cargo install trunk
```

## Develop

```sh
trunk serve --open
```

## Build (static site)

```sh
trunk build --release
```

The deployable static site is written to `dist/`.

## Data model

Ingredients are defined once in a central catalog (`ingredients.json`), keyed by
id, each with a single canonical unit and a shopping category:

```json
{
  "spaghetti": { "name": "Spaghetti", "unit": "g", "category": "Pasta & Grains" },
  "egg":       { "name": "Egg",       "unit": "unit", "category": "Dairy & Eggs" }
}
```

Recipes (`recipes/*.json`) reference catalog ingredients **by key** and give only
a number. Because every ingredient has one fixed unit, the shopping list sums
amounts exactly. Omit `amount` to mean "to taste":

```json
{
  "id": "carbonara",
  "title": "Spaghetti Carbonara",
  "servings": 2,
  "ingredients": [
    { "ingredient": "spaghetti", "amount": 200 },
    { "ingredient": "egg", "amount": 4 },
    { "ingredient": "black_pepper" }
  ],
  "steps": ["…"]
}
```

### Adding a recipe

1. If a recipe uses an ingredient not yet in `ingredients.json`, add it there first
   (pick its natural unit and category).
2. Add a `recipes/<name>.json` file referencing ingredients by key.
3. Rebuild. `cargo test` validates that every referenced key exists in the catalog.

(Both files are embedded into the wasm binary at compile time — see `src/recipes.rs`.)

## Deploy (GitHub Pages)

Pushing to `main` runs `.github/workflows/deploy.yml`, which builds with the
correct base path and publishes `dist/` to GitHub Pages.

One-time setup: in the repo, **Settings → Pages → Source → GitHub Actions**.

The base path assumes a project page at `https://<user>.github.io/cooking-rs/`.
If you rename the repo or use a user/root or custom-domain page, update **both**
`--public-url` and `APP_BASE_PATH` in the workflow (root page → `/` and `""`).

## Project layout

```
ingredients.json    ingredient catalog (embedded at build time)
recipes/            recipe data (embedded at build time)
images/             per-recipe images (copied into dist/ via index.html)
src/
  main.rs           entry point: mounts the app
  app.rs            router (base-path aware) + nav shell
  models.rs         Recipe / RecipeIngredient / IngredientCatalog / MealPlan
  recipes.rs        data loading boundary (embed today, fetch-ready) + validation tests
  filter.rs         recipe search + cuisine/tag filtering
  nutrition.rs      recipe/meal macro computation from the ingredient catalog
  format.rs         display helpers (timing, macros, ingredient lines, asset_url)
  shopping.rs       meal plan -> summed, category-grouped shopping list
  state.rs          meal-plan <-> URL-hash encoding
  pwa.rs            service-worker registration
  components/       shared UI (nav, recipe overview, add-to-planner, macro summary, modal)
  pages/            recipes, recipe detail, planner, and ingredients pages
```
