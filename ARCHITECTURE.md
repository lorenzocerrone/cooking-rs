# Architecture & Roadmap

`cooking-rs` is a fully static, single-page web app for browsing cooking recipes
and assembling a shareable weekly meal plan. It has **no backend and no database**:
all data lives in JSON files that are embedded into the WebAssembly binary at build
time, and all user state lives in the URL.

---

## 1. Goals & constraints

- **Statically buildable.** The output is a folder of static files (`dist/`) that any
  static host can serve — currently GitHub Pages.
- **No server, no database.** Recipes and ingredients are plain JSON checked into the repo.
- **Shareable state.** A meal plan is fully encoded in the URL, so copying the link
  shares the plan — no accounts, no storage.
- **Simple, modern Rust.** Leptos (client-side rendering) + Trunk, small focused modules.
- **Localized change.** Data loading and state encoding sit behind narrow boundaries so
  future changes (e.g. fetching recipes at runtime) touch one file.

## 2. Technology stack

| Concern        | Choice                                  |
| -------------- | --------------------------------------- |
| Language       | Rust (edition 2021)                     |
| UI framework   | [Leptos](https://leptos.dev) 0.8, CSR   |
| Routing        | `leptos_router` 0.8 (history routing)   |
| Build tool     | [Trunk](https://trunkrs.dev) 0.21       |
| Target         | `wasm32-unknown-unknown`                |
| Data embedding | `include_dir!` / `include_str!`         |
| Serialization  | `serde` + `serde_json`                  |
| State encoding | `base64` (URL-safe) in the URL fragment |
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
 │                               ├─ "/ingredients"→ IngredientsPage  │
 │                               └─ "/planner"    → PlannerPage      │
 │                                                                  │
 │  load_data() ─► Resource ─► <Suspense> ─► pages render           │
 │  App MealPlan signal (context) ⇄ URL #hash  (state.rs)          │
 │  pwa::setup() ─► registers service worker + manifest             │
 └──────────────────────────────────────────────────────────────────┘
```

## 4. Module guide

The code is organised into **layers**, each depending only on the one below. Nothing below the
component layer uses Leptos types, so all logic is unit-testable in isolation and the UI can be
reworked without touching logic (and vice versa):

```
models            domain types (pure)
  ↑
shopping/nutrition/filter   pure LOGIC (aggregation, macros, search) — returns data
  ↑
format            pure DISPLAY FORMATTING — turns data into strings (no Leptos)
  ↑
components/*       reusable presentational COMPONENTS (prop-driven)
  ↑
pages/*           PAGE composition — wires data/signals into components
```

```
ingredients.json    Central ingredient catalog (embedded). key → {name, unit, category, macros?}.
recipes/*.json      Recipes (embedded). Reference ingredients by key + amount.

src/
  main.rs           Entry point: panic hook, pwa::setup(), mounts <App> to the body.
  app.rs            <Router> (reads APP_BASE_PATH at compile time) + <Nav> + routes.
                    Owns the shared MealPlan signal (provide_context) and the URL-hash
                    sync effect. Exposes the pub BASE_PATH constant.
  models.rs         Domain types: Recipe (+ description/cuisine/tags/times/image/macros),
                    RecipeIngredient, CatalogIngredient (+ macros), IngredientCatalog,
                    Macros, MealPlan/DayPlan, MealEntry, Slot, DAY_NAMES.
  recipes.rs        Data-loading BOUNDARY. load_recipes()/load_catalog()/load_data()
                    returning AppData. Embedded today; async-first so a future HTTP
                    loader is a one-file change. Holds the ref-validation test.
  shopping.rs       Pure LOGIC: aggregate(plan, recipes, catalog) → category-grouped,
                    summed (servings-scaled) shopping list; CATEGORY_ORDER/category_sort_key.
  nutrition.rs      Pure LOGIC: recipe_macros() computes macros from the catalog (explicit
                    recipe macros win); meal_macros/day_macros/week_macros; per_serving/scale.
  filter.rs         Pure LOGIC: RecipeQuery + filter()/available_cuisines()/available_tags().
  format.rs         Pure DISPLAY FORMATTING (no Leptos): format_amount/ingredient_line/
                    format_text/format_kcal/format_macros/format_timing/total_time.
  state.rs          State BOUNDARY. encode/decode a MealPlan ⇄ base64 ⇄ URL #fragment.
  pwa.rs            Registers the service worker + injects the manifest <link>, using
                    BASE_PATH so they resolve on deep links under a subpath.
  components/
    nav.rs            Top navigation bar (active link via aria-current).
    modal.rs          Generic Modal (open signal; closes on backdrop/Esc/button).
    recipe_overview.rs RecipeOverview: shared summary (image, badges, per-serving macros,
                      ingredients) reused by the detail page AND the planner modal.
    macro_summary.rs  MacroSummary card (title, kcal, P/C/F, subtitle).
    add_to_planner.rs AddToPlanner: button + day/slot picker modal that writes the shared
                      MealPlan (via context); used on recipe cards and the detail page.
  pages/
    recipes_page.rs       Lists recipes; builds a RecipeQuery (filter.rs) from its signals;
                          cards carry an AddToPlanner control.
    recipe_detail_page.rs Single recipe (/recipe/:id): <RecipeOverview> + AddToPlanner + steps.
    ingredients_page.rs   Read-only view of the full catalog, grouped by category.
    planner_page.rs       Responsive grid of 7 day-cards (no horizontal scroll); per-meal
                          servings; ⓘ opens the overview modal; per-day + week MacroSummary;
                          shopping list with copy/check-off/omit. Reads the plan from context.
```

### Key design boundaries

- **`recipes.rs` — the data boundary.** Everything else consumes recipes/catalog through
  `load_data()`, which returns `AppData` and is `async`. Pages wrap it in
  `Resource` + `<Suspense>`. Switching from compile-time embedding to runtime HTTP fetch
  means rewriting only these loader bodies (and adding a `manifest.json`); pages are untouched.
- **`state.rs` — the sharing boundary.** The meal plan is the only shared state. It is
  serialized to JSON → base64 (URL-safe, no padding) → stored in the URL hash
  (`#plan=…`). `App` owns a single `RwSignal<MealPlan>` seeded from the hash, shares it via
  `provide_context`, and writes it back on every change via an effect (skipping the initial
  empty write). Pages consume it with `expect_context` — the planner edits it, and recipe
  pages add to it through `AddToPlanner` — so all views stay in sync and shareable.
- **The logic ⇄ UI boundary.** `shopping.rs`, `nutrition.rs`, and `filter.rs` are pure logic
  (no Leptos; take plain data, return plain data) and `format.rs` is pure formatting (data →
  strings). Components and pages never compute or format inline — they call into these layers.
  This keeps UI and logic independently changeable and makes the logic/format layers directly
  unit-testable (see the `#[cfg(test)]` modules in each).
- **Component reuse.** Presentation lives in `components/*` (e.g. `RecipeOverview` is shared by
  the detail page and the planner modal); pages only compose components and wire signals.

## 5. Data model

**Ingredient catalog** (`ingredients.json`) — one keyed entry per ingredient, each with a
single **canonical unit** and a shopping **category**, plus **optional `macros`** (per 100 of
the canonical unit for `g`/`ml`, or per 1 for `unit`):

```json
"egg": { "name": "Egg", "unit": "unit", "category": "Dairy & Eggs",
         "macros": { "calories": 70, "protein": 6, "carbs": 0.5, "fat": 5 } }
```

**Recipe** (`recipes/*.json`) — references catalog ingredients **by key** and supplies only
a number. Because each ingredient has exactly one unit, amounts sum with plain addition
(no unit conversion). Omitting `amount` means "to taste". Beyond the core fields, all of
`description`, `cuisine`, `tags`, `prep_time`, `cook_time`, `image`, and `macros` are
**optional** (`#[serde(default)]`), so older recipe files keep parsing. When `macros` is
omitted, `nutrition::recipe_macros` computes them from the ingredient catalog.

```json
{ "id": "carbonara", "title": "…", "servings": 2,
  "cuisine": "Italian", "tags": ["pasta"], "prep_time": 10, "cook_time": 15,
  "ingredients": [ { "ingredient": "egg", "amount": 4 },
                   { "ingredient": "black_pepper" } ],
  "steps": ["…"] }
```

**Meal plan** (in-memory + URL) — `MealPlan { days: [DayPlan; 7] }`, each `DayPlan` holding an
optional `MealEntry` for `breakfast`/`lunch`/`dinner`. A `MealEntry` is a recipe id plus an
optional target `servings` (which scales ingredient amounts before summing). To keep shared
links stable, a `MealEntry` (de)serializes as a **bare string** when no custom servings are set
(the original id-only format) and as `{"recipe":…,"servings":…}` only when servings are
customized — so **legacy `#plan=` links still decode**, and the URL stays independent of the
recipe/ingredient schema.

**Invariant:** every `RecipeIngredient.ingredient` must exist in the catalog. Enforced by
the `#[cfg(test)]` test in `recipes.rs` (runs under `cargo test` on the host).

## 6. Build, run, deploy

- **Dev:** `trunk serve --open` — compiles to wasm, serves at `/`, live-reloads on save.
- **Test:** `cargo test` — validates recipe↔catalog references on the host target.
- **CI:** `.github/workflows/ci.yml` runs on every push and pull request:
  `cargo fmt --all --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test` →
  `trunk build` (wasm). A green CI means a deployable tree.
- **Static build:** `trunk build --release` → `dist/`.
- **Deploy:** push to `main` → `.github/workflows/deploy.yml` builds with
  `--public-url /cooking-rs/` and `APP_BASE_PATH=/cooking-rs` (asset base *and* router base
  must match), copies `index.html → 404.html` for SPA deep-link fallback, and publishes to
  GitHub Pages. The base path is the one value to keep in sync if the repo is renamed or a
  root/custom-domain page is used (then `/` and `""`).
- **PWA assets:** `manifest.webmanifest`, `sw.js`, and `icons/` are copied into `dist/` via
  `data-trunk rel="copy-file"`/`"copy-dir"` directives in `index.html`. The manifest `<link>`
  and the service-worker registration are injected at runtime from `pwa::setup()` using
  `BASE_PATH`, so they stay correct on deep links under the subpath. The service worker is
  network-first with a cache fallback — after the first load the app works fully offline (all
  recipe/ingredient data is embedded in the wasm).

## 7. Roadmap

### Done
- **Phase 1 — Skeleton.** Leptos CSR + Trunk scaffold, two routes, URL-hash state sharing,
  embedded recipes via an async-first loader, plain CSS.
- **Phase 2 — Structured data + deploy.** Central keyed ingredient catalog, recipes
  referencing ingredients by key, summed category-grouped shopping list, reference-validation
  test, and GitHub Pages deployment.
- **Phase 3 — UX & schema.** Recipe schema polish (`description`, `cuisine`/`tags`,
  `prep_time`/`cook_time`, `image`); recipes-page search + tag/cuisine filtering; recipe detail
  view (`/recipe/:id`) with steps; ingredients catalog page (`/ingredients`); shopping-list
  polish (copy-to-clipboard, check-off items, omit "to taste" toggle).
- **Phase 4 — Scaling, macros, PWA.** Per-meal servings scaling via `MealEntry` (backward
  compatible with id-only links); macros per ingredient/recipe with `nutrition::recipe_macros`
  (computed from the catalog, explicit recipe macros win); installable PWA with an offline
  service worker.

### Later (bigger)
- **Unit conversion** — allow multiple compatible units per ingredient (g/kg, ml/cup) with a
  conversion layer; needed only if single-canonical-unit proves too rigid.
- **Runtime-fetched recipes** — swap the `recipes.rs` loader to fetch `recipes/*.json` +
  `ingredients.json` over HTTP (add a `manifest.json`) so data changes don't need a rebuild.
  The async-first boundary already anticipates this.

### Non-goals (for now)
User accounts, a backend/database, server-side rendering, and image hosting. The app stays a
static, link-shareable SPA.

---

> Update this document when a boundary, the data model, or the deploy flow changes — it is
> the map newcomers (human or AI) read first.
