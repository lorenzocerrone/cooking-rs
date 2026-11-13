# cooking-rs

A small, statically-buildable web app for organizing recipes, planning the coming
days' meals, and getting the grocery list for it. Written in Rust with
[Leptos](https://leptos.dev) (CSR) and built with [Trunk](https://trunkrs.dev).

No backend, no database — recipes are plain JSON files embedded at compile time;
the plan lives in the address bar, so sharing it is sending the link.

## Features

- **Recipes** — every `recipes/*.json` file, with search and a filter sheet
  (vegetarian or not, author, cuisine, time, tags; pick several at once); the
  filters stay in the URL, so a filtered list can be sent too; a full detail page
  with steps.
- **Recipe editor** — a form for writing a recipe on the phone: pick ingredients from
  a list of common ones or type any other, choose amounts and units, write the steps; it says
  what is still missing and previews the recipe as the real page. "Paste a recipe"
  fills it in from text copied from a website or a message.
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

### Recipes (`recipes/*.json`)

A recipe is self-contained: each ingredient line names its ingredient as written.
`unit` may be any of `g kg ml l tsp tbsp cup pcs`, the ingredient's piece label
(`cloves`, `can`) or any piece word (`bunch`); omit it for a plain count ("2" eggs)
and omit `amount` for "to taste".

```json
{
  "id": "lentil_dal",
  "title": "Lentil Dal",
  "author": "Lorenzo",
  "servings": 4,
  "cuisine": "Indian",
  "tags": ["dinner"],
  "prep_time": 10,
  "cook_time": 30,
  "image": "images/lentil_dal.svg",
  "ingredients": [
    { "name": "Lentils", "amount": 300, "unit": "g" },
    { "name": "Onion", "amount": 1, "note": "finely chopped" },
    { "name": "Garlic", "amount": 3, "unit": "cloves" },
    { "name": "Olive oil", "amount": 1, "unit": "tbsp" },
    { "name": "Star anise", "amount": 2, "aisle": "Spices & Oils" },
    { "name": "Salt" }
  ],
  "steps": ["…"]
}
```

### Ingredient dictionary (`ingredients.json`)

Knowledge about common ingredients, looked up by name (key, name or an alias; any
case, singular or plural). A known ingredient gets its grocery aisle, unit
conversions and merging across recipes. `unit` is the **shopping unit** (`g`, `ml`
or `pcs`), and `count` says how it is counted in pieces:

```json
{
  "onion":    { "name": "Onion",    "unit": "g", "category": "Produce",
                "count": { "per_piece": 150 } },
  "garlic":   { "name": "Garlic",   "unit": "g", "category": "Produce",
                "count": { "label": "clove", "per_piece": 4 } },
  "scallion": { "name": "Scallion", "unit": "g", "category": "Produce",
                "count": { "per_piece": 15 }, "aliases": ["spring onion", "green onion"] }
}
```

Any other name works too, on its own grocery line (in its `aisle`, or "Other").
`cargo run --bin ingredients` lists the names the dictionary doesn't know (with near
matches) and amounts the grocery list can't add up; adding an alias is usually the fix.

JSON Schemas for both files live in `schema/` and are wired up for VS Code in
`.vscode/settings.json` (autocomplete and inline errors while editing).

### Adding a recipe

The easy way: in Claude Code in this repo, paste or dictate a recipe, attach a photo
of a recipe card, or send a link (or `/add-recipe`). The `add-recipe` skill
(`.claude/skills/add-recipe/`) matches it to the dictionary, runs the scaffolder below,
shows one preview to confirm, runs the tests, commits, and asks before pushing. It
also changes existing recipes ("make the carbonara serve 4").

In the app, **Recipes → New** opens a form that builds the same recipe JSON (plus any
with a live preview.

By hand, write a plain-text draft:

```text
Lentil Dal
author: Lorenzo
servings: 4
cuisine: Indian
tags: dinner
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
(`g kg ml l tsp tbsp cup pcs`), the ingredient's piece label (`cloves`, `can`) or
a common piece word (`bunch`, `sprig`…). A bare number counts pieces ("2 onion" is
two onions), text after a comma is a note, and a line without an amount is "to
taste". Any ingredient name works. Every recipe needs an `author:` (the bundled
ones say "Claude"), and the filter sheet gets an Author section as soon as there
are two authors. The scaffolder sets the `vegetarian` tag when the dictionary knows
every ingredient and none is meat or fish (otherwise write it yourself), and drops
it when one is. It writes `recipes/<id>.json` and a placeholder `images/<id>.svg`;
on a malformed line it lists every problem by line and writes nothing. Names the
dictionary doesn't know and units the grocery list can't add up are warnings.

## Deploy (GitHub Pages)

Pushing to `main` runs `.github/workflows/deploy.yml`, which builds with the
correct base path and publishes `dist/` to the root of the `gh-pages` branch.

Every pull request gets a preview: `.github/workflows/preview.yml` builds it to
`https://<user>.github.io/cooking-rs/pr-preview/pr-<number>/`, comments the link
on the PR, rebuilds on each push and removes it when the PR closes. A preview says
"preview · pr-N" under the logo, keeps its own saved plan and drafts, and has no
offline service worker, so trying it can't touch the live app's data.

One-time setup: in the repo, **Settings → Pages → Build and deployment → Source:
Deploy from a branch → `gh-pages`, `/ (root)`**. The branch appears after the
first deploy from `main`.

The base path assumes a project page at `https://<user>.github.io/cooking-rs/`.
If you rename the repo or use a user/root or custom-domain page, update **both**
`--public-url` and `APP_BASE_PATH` in the workflow (root page → `/` and `""`).

## Project layout

```
ingredients.json    ingredient dictionary (embedded at build time)
recipes/            recipe data (embedded at build time)
images/             per-recipe images (copied into dist/ via index.html)
schema/             JSON Schemas for the two data files (editor support)
build.rs            re-runs the build when recipes/ or the catalog change
src/
  lib.rs            the crate's modules, shared by both binaries
  main.rs           wasm entry point: mounts the app
  bin/scaffold.rs   host-only recipe scaffolder (cargo run --bin scaffold)
  scaffold.rs       draft text -> validated Recipe, JSON + placeholder SVG (pure)
  editor.rs         the editor's form -> validated Recipe (pure)
  report.rs         how ingredient names fit the dictionary (cargo run --bin ingredients)
  paste.rs          pasted recipe text -> a filled-in form (pure)
  app.rs            router + nav shell; owns the shared plan + grocery state
  models.rs         Recipe / RecipeIngredient / CatalogIngredient / MealPlan
  quantity.rs       units, dimensions, piece specs, conversions (pure)
  recipes.rs        data loading boundary (embed today, fetch-ready)
  validate.rs       data validation tests (cargo test)
  filter.rs         recipe search + multi-select filters, URL query (pure)
  shopping.rs       meal plan -> converted, summed, aisle-grouped grocery lines (pure)
  format.rs         display strings: quantities, rounding, text export (pure)
  state.rs          meal plan <-> URL hash (the address is the plan)
  storage.rs        last plan + grocery ticks <-> localStorage
  pwa.rs            service-worker registration
  components/       nav, modal, recipe overview, recipe picker, add-to-planner, icons
  pages/            recipes, recipe detail, planner, grocery
```
