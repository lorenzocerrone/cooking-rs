# Architecture & Roadmap

`cooking-rs` is a fully static, single-page web app for organizing recipes,
planning a week of meals, and producing the grocery list for it. It has **no
backend and no database**: recipe data lives in JSON files embedded into the
WebAssembly binary at build time; user state lives in the browser (`localStorage`)
and, for sharing, in the URL.

---

## 1. Goals & constraints

- **Statically buildable.** The output is a folder of static files (`dist/`) that any
  static host can serve — currently GitHub Pages.
- **No server, no database.** Recipes and ingredients are plain JSON checked into the repo.
- **Local-first, link-shareable.** The meal plan and grocery ticks persist on the device.
  The plan is also mirrored into the URL, so copying the link shares it — no accounts.
- **Correct grocery math.** Recipes use natural units ("1 onion", "3 cloves", "2 tbsp",
  "1 can"); the list converts into each ingredient's shopping unit and sums exactly.
- **Validated data.** `cargo test` rejects any recipe or catalog file that would degrade the
  app (unparseable, unknown ingredient, unconvertible unit, missing image).
- **Simple, modern Rust.** Leptos (client-side rendering) + Trunk, small focused modules,
  logic kept free of UI types so it is unit-testable on the host.

## 2. Technology stack

| Concern        | Choice                                  |
| -------------- | --------------------------------------- |
| Language       | Rust (edition 2021)                     |
| UI framework   | [Leptos](https://leptos.dev) 0.8, CSR   |
| Routing        | `leptos_router` 0.8 (history routing)   |
| Build tool     | [Trunk](https://trunkrs.dev) 0.21       |
| Target         | `wasm32-unknown-unknown`                |
| Data embedding | `include_dir!` / `include_str!` (+ `build.rs` change tracking) |
| Serialization  | `serde` + `serde_json`                  |
| Share links    | `base64` (URL-safe) in the URL fragment |
| Persistence    | `localStorage` via `web-sys`            |
| Hosting        | GitHub Pages via GitHub Actions         |

## 3. High-level architecture

```
            ┌──────────────────── build time ────────────────────┐
 ingredients.json ─┐                                              │
 recipes/*.json  ──┼─► include_dir!/include_str!  ─►  wasm binary │
 style.css       ──┘            (embedded data)                   │
            └─────────────────────────────────────────────────────┘
                                   │  trunk build
                                   ▼
                              dist/ (static)
                                   │  served by GitHub Pages
                                   ▼
 ┌──────────────────────── browser (runtime) ───────────────────────┐
 │  main.rs  ── mounts ──►  App (app.rs)                             │
 │                          ├─ Router (base-path aware)             │
 │                          ├─ Nav                                  │
 │                          └─ Routes                               │
 │                               ├─ "/"           → RecipesPage      │
 │                               ├─ "/recipe/:id" → RecipeDetailPage │
 │                               ├─ "/planner"    → PlannerPage      │
 │                               └─ "/grocery"    → GroceryPage      │
 │                                                                  │
 │  load_data() ─► Resource ─► <Suspense> ─► pages render           │
 │  App: MealPlan + GroceryState signals (context)                  │
 │       ⇄ localStorage (storage.rs)   MealPlan ⇄ URL #hash (state.rs) │
 │  pwa::setup() ─► registers service worker + manifest             │
 └──────────────────────────────────────────────────────────────────┘
```

## 4. Module guide

The code is organised into **layers**, each depending only on the ones below. Nothing below
the component layer uses Leptos types, so all logic is unit-testable in isolation and the UI
can be reworked without touching logic (and vice versa):

```
quantity          units, dimensions, piece specs, conversions (pure, no deps)
  ↑
models            domain types (pure)
  ↑
shopping/filter   pure LOGIC (aggregation, search) — returns data
  ↑
format            pure DISPLAY FORMATTING — turns data into strings (no Leptos)
  ↑
state/storage     encoding boundaries (pure core + thin window wrappers)
  ↑
components/*      reusable presentational COMPONENTS (prop-driven)
  ↑
pages/*           PAGE composition — wires data/signals into components
```

```
ingredients.json    Central ingredient catalog (embedded). key → {name, unit, category, count?}.
recipes/*.json      Recipes (embedded). Reference ingredients by key + amount (+ unit, note, optional).
schema/*.json       JSON Schemas for the two files (editor autocomplete; tests are the truth).
build.rs            `rerun-if-changed` for recipes/ and the catalog, so adding/removing a file
                    is picked up by `cargo test` and `trunk serve` (include_dir! alone isn't).

src/
  main.rs           Entry point: panic hook, pwa::setup(), mounts <App> to the body.
  app.rs            <Router> (reads APP_BASE_PATH at compile time) + <Nav> + routes.
                    Owns the shared MealPlan and GroceryState signals (provide_context),
                    seeds them via storage::resolve_initial, and runs the two sync effects
                    (localStorage save; URL-hash mirror). Exposes BASE_PATH and use_grocery().
  quantity.rs       Dimension {Mass, Volume, Count}; Unit {g, kg, ml, l, tsp, tbsp, cup, pcs}
                    with parse/as_str/to_base; Quantity; CountSpec {label, plural, per_piece};
                    resolve_unit (recipe text → Unit, incl. piece labels); to_shopping
                    (Quantity → ingredient's shopping unit, or Other(dimension) if no conversion).
  models.rs         Domain types: Recipe, RecipeIngredient {ingredient, amount?, unit?, note?,
                    optional}, CatalogIngredient {name, unit: Unit, category, count?},
                    IngredientCatalog, MealPlan {days, extras} / DayPlan / MealEntry / Slot.
                    All data structs deny unknown fields.
  recipes.rs        Data-loading BOUNDARY. recipe_files() (per-file parse results),
                    load_recipes()/parse_catalog()/load_catalog()/load_data() → AppData.
                    Embedded today; async-first so a future HTTP loader is a one-file change.
  validate.rs       #[cfg(test)] data validation: files parse, ids match file names, refs
                    resolve, units convert, catalog units are base units, images exist.
  shopping.rs       Pure LOGIC: aggregate(plan, recipes, catalog) → category-grouped
                    ShoppingLines {key, name, count, subtotals (base units, primary first),
                    to_taste, optional, sources}; extras folded in unscaled.
  filter.rs         Pure LOGIC: RecipeQuery + filter()/available_cuisines()/available_tags().
  format.rs         Pure DISPLAY FORMATTING (no Leptos): format_quantity (authored unit),
                    format_ingredient_amount, format_subtotal (kg/l step-up, whole pieces,
                    labels), format_line_quantity, format_text (export), timing, asset_url.
  state.rs          Share BOUNDARY. encode/decode a MealPlan ⇄ base64 ⇄ URL #fragment;
                    decode normalizes to 7 days.
  storage.rs        Persistence BOUNDARY. PersistedState {version, plan, grocery} ⇄ JSON ⇄
                    localStorage; GroceryState {checked (with amount snapshots), omitted,
                    hide_to_taste}; resolve_initial (hash wins); prune_checked.
  pwa.rs            Registers the service worker + injects the manifest <link> (prod only).
  components/
    nav.rs            Top navigation bar (active link via aria-current).
    modal.rs          Generic Modal (open signal; closes on backdrop/Esc/button).
    icons.rs          Inline SVG icons.
    recipe_overview.rs RecipeOverview: image, badges, ingredient table (amount via
                      format_ingredient_amount, note, optional); shared by detail + modals.
    recipe_picker.rs  Searchable/filterable recipe picker modal for a planner slot.
    add_to_planner.rs AddToPlanner: button + day/slot picker that writes the shared plan.
  pages/
    recipes_page.rs       Lists recipes with search/filters and a quick-view modal.
    recipe_detail_page.rs Single recipe (/recipe/:id): <RecipeOverview> + AddToPlanner + steps.
    planner_page.rs       7 day-cards × 3 slots (chips, servings, picker), fill/clear toolbar.
    grocery_page.rs       Aggregated list with ticks, "have it" omissions, to-taste toggle,
                          copy/export, and an extras editor.
```

### Key design boundaries

- **`recipes.rs` — the data boundary.** Everything consumes recipes/catalog through
  `load_data()` (async, `Resource` + `<Suspense>`). Switching to runtime HTTP fetch means
  rewriting only the loader bodies; pages are untouched.
- **`quantity.rs` — the unit boundary.** The only place that knows what a tablespoon is.
  Recipes keep `unit` as text (labels like "clove" need the catalog to resolve); the catalog's
  `unit` is a typed `Unit`. Conversion always targets the ingredient's shopping unit; when
  none exists the amount is kept in its own dimension rather than dropped.
- **`state.rs` / `storage.rs` — the sharing and persistence boundaries.** Pure encode/decode
  cores with host tests; the `window` access is two small wrappers. Startup precedence: a
  plan in the URL hash (a shared link) wins, the device's grocery state is kept, and the merge
  is saved immediately. Both payloads are untagged JSON with defaulted fields, so every link
  ever shared keeps decoding; `storage` carries a `version` for future breaking changes.
- **The logic ⇄ UI boundary.** `shopping.rs` and `filter.rs` are pure logic and `format.rs`
  is pure formatting. Components and pages never compute or format inline.

## 5. Data model

**Ingredient catalog** (`ingredients.json`) — one keyed entry per ingredient with a **shopping
unit** (`g`, `ml` or `pcs`), an aisle `category`, and an optional `count` describing pieces:

```json
"onion":  { "name": "Onion",  "unit": "g",   "category": "Produce", "count": { "per_piece": 150 } },
"garlic": { "name": "Garlic", "unit": "g",   "category": "Produce", "count": { "label": "clove", "per_piece": 4 } },
"egg":    { "name": "Egg",    "unit": "pcs", "category": "Dairy & Eggs", "count": { "per_piece": 55 } },
"chopped_tomatoes": { "name": "Chopped tomatoes", "unit": "pcs", "category": "Pantry",
                      "count": { "label": "can", "per_piece": 400 } }
```

`per_piece` is one piece expressed in the shopping unit (for a `pcs` ingredient: grams per
piece, so "100 g" of it can still be converted). The author decides how a thing is bought:
onion in grams (1 onion → 150 g) or canned tomatoes by the can (400 g → 1 can).

**Recipe** (`recipes/*.json`) — references catalog ingredients by key. Each ingredient has an
optional `amount` (absent = "to taste"), `unit` (any `Unit` spelling or the ingredient's piece
label; absent = shopping unit), `note`, and `optional`. Everything beyond the core fields is
optional and old files keep parsing; unknown fields are rejected (typos fail validation).

```json
{ "id": "lentil_dal", "title": "Lentil Dal", "servings": 4, "image": "images/lentil_dal.svg",
  "ingredients": [ { "ingredient": "onion", "amount": 1, "unit": "pcs", "note": "finely chopped" },
                   { "ingredient": "garlic", "amount": 3, "unit": "cloves" },
                   { "ingredient": "olive_oil", "amount": 1, "unit": "tbsp" },
                   { "ingredient": "salt" } ],
  "steps": ["…"] }
```

**Grocery aggregation** — for each contribution: resolve the unit, scale by servings,
`to_shopping` it, add into the line's per-dimension totals. A line shows its primary
(shopping-unit) sub-total first and any unconvertible sub-total after ("2 pcs + 100 g").
Display rounds to shop-friendly values (kg/l above 1000, whole pieces rounded up).

**Meal plan** (in-memory, URL, localStorage) — `MealPlan { days: [DayPlan; 7], extras }`.
Each `DayPlan` holds an optional `MealEntry` (recipe id + optional servings) per slot; a
`MealEntry` serializes as a bare string when servings are default (legacy format) and as an
object otherwise. `extras` are `RecipeIngredient`s for staples not tied to a meal; omitted when
empty, so plans without them serialize exactly as before. `normalize()` pads decoded plans
to 7 days.

**Persisted state** (localStorage key `cooking-rs.state`) —
`{ version, plan, grocery: { checked: [{key, subtotals}], omitted: [key], hide_to_taste } }`.
A tick remembers the amounts it was made at and is dropped when the plan changes them;
omissions are sticky.

**Invariants** (enforced by `src/validate.rs` under `cargo test`): every data file parses;
recipe `id` equals the file stem and is unique; every ingredient key exists; every unit
resolves and converts into the ingredient's shopping unit; catalog units are base units;
every recipe image exists on disk.

## 6. Build, run, deploy

- **Dev:** `trunk serve --open` — compiles to wasm, serves at `/`, live-reloads on save.
  The service worker is not registered on localhost (and self-unregisters), so dev never
  serves stale assets.
- **Test:** `cargo test` — unit tests for every pure module plus the data validation suite.
- **CI:** `.github/workflows/ci.yml` runs on every push and pull request:
  `cargo fmt --all --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test` →
  `trunk build` (wasm). A green CI means a deployable tree.
- **Static build:** `trunk build --release` → `dist/`.
- **Deploy:** push to `main` → `.github/workflows/deploy.yml` builds with
  `--public-url /cooking-rs/` and `APP_BASE_PATH=/cooking-rs` (asset base *and* router base
  must match), copies `index.html → 404.html` for SPA deep-link fallback, and publishes to
  GitHub Pages. Requires Pages to be enabled with **Source: GitHub Actions** in the repo
  settings. The base path is the one value to keep in sync if the repo is renamed or a
  root/custom-domain page is used (then `/` and `""`).
- **PWA assets:** `manifest.webmanifest`, `sw.js`, and `icons/` are copied into `dist/` via
  `data-trunk` directives in `index.html`; the service worker is network-first with a cache
  fallback and only caches successful same-origin responses.

## 7. Roadmap

### Done
- **Phase 1 — Skeleton.** Leptos CSR + Trunk scaffold, routes, URL-hash state sharing,
  embedded recipes via an async-first loader, plain CSS.
- **Phase 2 — Structured data + deploy.** Central keyed ingredient catalog, recipes
  referencing ingredients by key, summed category-grouped shopping list, GitHub Pages workflow.
- **Phase 3 — UX & schema.** Recipe metadata (`description`, `cuisine`/`tags`, times, `image`);
  search + filtering; recipe detail view; shopping-list copy/check-off/omit "to taste".
- **Phase 4 — Scaling + PWA.** Per-meal servings (backward-compatible links); installable PWA
  with an offline service worker; planner chips, recipe picker, quick-view.
- **Phase 5 — Refocus on the core (recipes · planner · grocery).** Nutrition removed. Unit
  model with piece conversions; stricter schemas and a data validation suite; grocery
  aggregation with sub-totals, sources, optional/to-taste flags and extras; localStorage
  persistence with tick pruning and "have at home"; dedicated grocery page.

### Next
- **UX pass.** Visual redesign now that the core is stable: grocery page layout, mobile
  planner, shared-link banner, "why is this on the list" (sources) tooltip.
- **More recipes and catalog coverage** (densities for "1 cup flour" are a one-arm extension
  of `to_shopping`).

### Later (bigger)
- **Runtime-fetched recipes** — swap the `recipes.rs` loader to fetch JSON over HTTP so data
  changes don't need a rebuild. The async-first boundary already anticipates this.
- **In-app recipe editing** on top of the same validated schema (export/import JSON).

### Non-goals (for now)
User accounts, a backend/database, server-side rendering, nutrition tracking, and image
hosting. The app stays a static, link-shareable SPA.

---

> Update this document when a boundary, the data model, or the deploy flow changes — it is
> the map newcomers (human or AI) read first.
