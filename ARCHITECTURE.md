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
  A "Share" button produces a link carrying the plan in the URL fragment — no accounts.
- **Phone first.** Designed for a phone in the kitchen and in the shop (bottom tab bar,
  bottom sheets, tap-to-tick rows); wider screens get a top bar and a 7-column week.
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
 │                          ├─ Nav (tab bar / top bar)              │
 │                          ├─ Routes                               │
 │                          │    ├─ "/"           → RecipesPage      │
 │                          │    ├─ "/recipe/:id" → RecipeDetailPage │
 │                          │    ├─ "/planner"    → PlannerPage      │
 │                          │    └─ "/grocery"    → GroceryPage      │
 │                          └─ Toast                                │
 │                                                                  │
 │  App: one Resource<AppData> (load_data) + MealPlan, GroceryState,│
 │       own_plan (share preview) and toast signals, all in context │
 │       ⇄ localStorage (storage.rs)   share link ⇄ #plan= (state.rs) │
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
shopping/filter/  pure LOGIC (aggregation, search, extras parsing) — returns data
extras
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
  app.rs            <Router> (reads APP_BASE_PATH at compile time) + <Nav> + routes + <Toast>.
                    Owns the shared signals (MealPlan, GroceryState, own_plan, toast) and the
                    single data Resource (provide_context), seeds them via
                    storage::resolve_initial, clears an imported #plan= hash, and saves to
                    localStorage on change (paused while a shared plan is previewed).
                    Exposes BASE_PATH, href() (every in-app link must use it: the router
                    base applies to matching only), use_plan/use_grocery/use_own_plan/
                    use_app_data and notify().
  quantity.rs       Dimension {Mass, Volume, Count}; Unit {g, kg, ml, l, tsp, tbsp, cup, pcs}
                    with parse/as_str/to_base; Quantity; CountSpec {label, plural, per_piece};
                    resolve_unit (recipe text → Unit, incl. piece labels); to_shopping
                    (Quantity → ingredient's shopping unit, or Other(dimension) if no conversion).
  models.rs         Domain types: Recipe, RecipeIngredient {ingredient, amount?, unit?, note?,
                    optional}, CatalogIngredient {name, unit: Unit, category, count?},
                    IngredientCatalog, MealPlan {days, extras} / DayPlan {meals} / MealEntry /
                    Extra {Catalog | Custom}. All data structs deny unknown fields.
  recipes.rs        Data-loading BOUNDARY. recipe_files() (per-file parse results),
                    load_recipes()/parse_catalog()/load_catalog()/load_data() → AppData.
                    Embedded today; async-first so a future HTTP loader is a one-file change.
  validate.rs       #[cfg(test)] data validation: files parse, ids match file names, refs
                    resolve, units convert, catalog units are base units, images exist.
  shopping.rs       Pure LOGIC: aggregate(plan, recipes, catalog) → category-grouped
                    ShoppingLines {key, name, count, subtotals (base units, primary first),
                    to_taste, optional, sources, custom_quantities}; catalog extras folded
                    in unscaled, custom extras as `custom:*` lines in the "Other" aisle (last).
  filter.rs         Pure LOGIC: RecipeQuery + filter()/available_cuisines()/available_tags().
  extras.rs         Pure LOGIC for the free-text extras line: parse_extra ("milk 1 l" →
                    name + quantity), suggest (catalog matches), to_extra (catalog item when
                    the unit converts, otherwise a custom item kept as typed).
  format.rs         Pure DISPLAY FORMATTING (no Leptos): format_quantity (authored unit),
                    format_ingredient_amount(_scaled), format_subtotal (kg/l step-up, whole
                    pieces, labels), format_line_quantity, format_text (export), timing.
  state.rs          Share BOUNDARY. encode/decode a MealPlan ⇄ base64 ⇄ `#plan=` fragment;
                    share_url builds the link, read_hash/clear_hash handle an opened one.
  storage.rs        Persistence BOUNDARY. PersistedState {version, plan, grocery} ⇄ JSON ⇄
                    localStorage; GroceryState {checked (with amount snapshots), omitted,
                    hide_to_taste} + visible()/remaining(); resolve_initial → Startup
                    {state, own_plan} (a hash plan is previewed, the own plan set aside);
                    prune_checked.
  pwa.rs            Registers the service worker + injects the manifest <link> (prod only).
  components/
    nav.rs            Recipes / Week / Grocery tabs: bottom bar on phones, top bar ≥720px
                      (CSS); grocery badge = items left; hidden on the recipe page.
    modal.rs          Generic Modal: bottom sheet on phones, dialog on desktop (CSS);
                      optional title; closes on backdrop/Esc/button.
    toast.rs          One-line confirmation set via app::notify, auto-hides.
    stepper.rs        "− n +" control (servings).
    recipe_filters.rs Search box + chip row (tags, cuisines) driving a RecipeQuery.
    recipe_picker.rs  Recipe picker sheet for a day; appends to the plan.
    add_to_week.rs    "Add to week" button + day sheet (recipe page).
    icons.rs          Inline SVG icons.
    mod.rs            also today_index() (Monday = 0, from the browser clock).
  pages/
    recipes_page.rs       Compact list with search/filter chips; rows link to the recipe.
    recipe_detail_page.rs Single recipe (/recipe/:id): servings stepper rescales ingredients,
                          tappable steps (not persisted), AddToWeek.
    planner_page.rs       "This week": Share button, shared-plan banner, 7 day cards (meal
                          rows + "Add a meal"), meal sheet (servings, view, remove), clear.
    grocery_page.rs       Aisles with tap-to-tick rows (ticked sink), swipe/hover "Have it",
                          at-home chips, free-text extras with suggestions, to-taste toggle,
                          copy.
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
  cores with host tests; the `window` access is a few small wrappers. Startup: a plan in the
  URL hash (a share link) is **previewed** — the device's own plan is set aside in `own_plan`,
  nothing is saved, the hash is cleared, and the planner shows a banner with "Keep it" (adopt
  and resume saving) or "Back to mine" (restore). Without a hash, the stored state loads.
  Both payloads are untagged JSON with defaulted fields and backward-compatible decoders, so
  every link ever shared keeps decoding; `storage` carries a `version` (now 2) so an older
  build refuses a newer payload instead of misreading it.
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

**Meal plan** (in-memory, share link, localStorage) — `MealPlan { days: [DayPlan; 7], extras }`.
A `DayPlan` is a plain **list** of `MealEntry` (recipe id + optional servings) in the order
added — no fixed breakfast/lunch/dinner slots. On the wire a day is a JSON array; the decoder
also accepts the original `{breakfast, lunch, dinner}` object (slots become list entries). A
`MealEntry` serializes as a bare string when servings are default and as an object otherwise.
`extras` are `Extra`s: `Catalog(RecipeIngredient)` (the pre-existing shape, summed like recipe
lines) or `Custom {name, quantity?}` (free text, shown as typed in the "Other" aisle);
omitted when empty. `normalize()` pads decoded plans to 7 days.

**Persisted state** (localStorage key `cooking-rs.state`) —
`{ version: 2, plan, grocery: { checked: [{key, subtotals}], omitted: [key], hide_to_taste } }`.
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
  fallback and only caches successful same-origin responses. Fonts (Space Grotesk, Space
  Mono) come from Google Fonts and fall back to system faces offline.
- **Styling:** plain CSS in `style.css`, tokens on `:root` (light) overridden under
  `prefers-color-scheme: dark`. Phone first: the tab bar, sheets and row interactions are
  the base; `min-width: 720px` (top bar, dialogs) and `900px` (7-column week) widen it.

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

- **Phase 6 — UX pass (phone first).** Parchment/sienna look in the family of `panetti`;
  bottom tab bar with grocery badge; compact recipe list; recipe page with servings stepper
  and tappable steps; days as meal lists with sheets for picking and editing; grocery rows
  with tap-to-tick and swipe "Have it", free-text extras with catalog suggestions; share
  button + shared-plan preview banner; dark mode.

### Next
- **Reorder meals within a day** and move a meal to another day.
- **Real photos** (the recipe page has no image until then; `image` stays validated).
- **"Why is this on the list"** (sources) on a grocery row.
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
