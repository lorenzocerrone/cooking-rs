# Architecture & Roadmap

`cooking-rs` is a fully static, single-page web app for organizing recipes,
planning the coming days' meals, and producing the grocery list for it. It has **no
backend and no database**: recipe data lives in JSON files embedded into the
WebAssembly binary at build time; the meal plan lives in the URL (the address bar
always carries it), and the browser (`localStorage`) remembers the last plan and the
grocery ticks.

---

## 1. Goals & constraints

- **Statically buildable.** The output is a folder of static files (`dist/`) that any
  static host can serve — currently GitHub Pages.
- **No server, no database.** Recipes and ingredients are plain JSON checked into the repo.
- **The URL is the plan.** A family shares a plan by sending the address: the plan is in
  the URL fragment and rewritten on every change, so a bookmark, the address bar and the
  Share button all give the same link — no accounts. The device remembers the last plan
  opened (for the home-screen icon) and its own grocery ticks.
- **Phone first.** Designed for a phone in the kitchen and in the shop (bottom tab bar,
  bottom sheets, tap-to-tick rows); wider screens get a top bar; the plan is one rolling list of dates everywhere.
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
| Last plan, ticks | `localStorage` via `web-sys`          |
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
 │       today and toast signals, all in context                    │
 │       ⇄ localStorage (storage.rs)  UrlSync: plan ⇄ #plan= (state) │
 │  pwa::setup() ─► registers service worker + manifest             │
 └──────────────────────────────────────────────────────────────────┘
```

## 4. Module guide

The code is organised into **layers**, each depending only on the ones below. Nothing below
the component layer uses Leptos types, so all logic is unit-testable in isolation and the UI
can be reworked without touching logic (and vice versa):

```
quantity/date     units and conversions; calendar dates (pure, no deps)
  ↑
models            domain types (pure)
  ↑
shopping/filter/  pure LOGIC (aggregation, search, extras parsing, recipe
extras/scaffold   scaffolding) — returns data
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
  lib.rs            The crate's modules (`pub mod …`), shared by the two binaries below.
  main.rs           Wasm entry point: panic hook, pwa::setup(), mounts <App> to the body.
  bin/scaffold.rs   Host-only authoring CLI (arguments + file I/O around scaffold.rs).
                    Trunk builds only the app (`data-bin="cooking-rs"` in index.html).
  app.rs            <Router> (reads APP_BASE_PATH at compile time) + <Nav> + routes + <Toast>.
                    Owns the shared signals (MealPlan, GroceryState, today, toast) and the
                    single data Resource (provide_context), seeds them via
                    storage::resolve_initial and saves to localStorage on change. UrlSync
                    rewrites #plan= after every plan change and navigation (deferred one
                    tick: the router pushes the new URL after updating its signals).
                    `today` rolls over on visibilitychange, dropping the day that passed.
                    Exposes BASE_PATH, href() (every in-app link must use it: the router
                    base applies to matching only), use_plan/use_grocery/
                    use_today/use_app_data and notify().
  date.rs           Date (days since 1970, ISO `YYYY-MM-DD` on the wire): from_ymd/ymd,
                    add_days, days_until, weekday (Monday = 0), next_weekday. No
                    dependency, and never reads the clock: "today" comes from the UI layer.
  quantity.rs       Dimension {Mass, Volume, Count}; Unit {g, kg, ml, l, tsp, tbsp, cup, pcs}
                    with parse/as_str/to_base; Quantity; CountSpec {label, plural, per_piece};
                    resolve_unit (recipe text → Unit, incl. piece labels); to_shopping
                    (Quantity → ingredient's shopping unit, or Other(dimension) if no conversion).
  models.rs         Domain types: Recipe, RecipeIngredient {ingredient, amount?, unit?, note?,
                    optional}, CatalogIngredient {name, unit: Unit, category, count?},
                    IngredientCatalog, MealPlan {dates: BTreeMap<Date, DayPlan>, extras} / PlanWire
                    (dated or legacy weekday wire shape → into_plan(today)) / DayPlan {meals} /
                    MealEntry /
                    Extra {Catalog | Custom}. All data structs deny unknown fields.
  recipes.rs        Data-loading BOUNDARY. recipe_files() (per-file parse results),
                    load_recipes()/parse_catalog()/load_catalog()/load_data() → AppData.
                    Embedded today; async-first so a future HTTP loader is a one-file change.
  validate.rs       #[cfg(test)] data validation: files parse, ids match file names, refs
                    resolve, units convert, catalog units are base units, images exist.
  shopping.rs       Pure LOGIC: aggregate(plan, from, to, recipes, catalog) → the meals dated
                    from..=to (extras always) as category-grouped
                    ShoppingLines {key, name, count, subtotals (base units, primary first),
                    to_taste, optional, sources, custom_quantities}; catalog extras folded
                    in unscaled, custom extras as `custom:*` lines in the "Other" aisle (last).
  filter.rs         Pure LOGIC: RecipeQuery {search, cuisine, tag, author} + filter() and
                    available_cuisines()/available_tags()/available_authors().
  extras.rs         Pure LOGIC for the free-text extras line: parse_extra ("milk 1 l" →
                    name + quantity), suggest (catalog matches), to_extra (catalog item when
                    the unit converts, otherwise a custom item kept as typed).
  scaffold.rs       Pure LOGIC for the scaffolder: parse_draft (title, `key: value` headers,
                    ingredient lines, "Steps") → Recipe; parse_line ("1 red onion", "3 cloves
                    garlic", "1/2 can coconut milk, note (optional)"; a bare number counts
                    pieces; units must convert); find_ingredient (key/name, singular/plural,
                    no guessing); unknown ingredients get extras::suggest hints and a
                    catalog stub; render_json (house style) and render_svg (placeholder).
  format.rs         Pure DISPLAY FORMATTING (no Leptos): format_day(_inline) ("Today",
                    "Tomorrow", "Mon 27 Oct"), format_range, format_quantity (authored unit),
                    format_ingredient_amount(_scaled), format_subtotal (kg/l step-up, whole
                    pieces, labels), format_line_quantity, format_text (export), timing.
  state.rs          URL BOUNDARY. encode/decode(…, today) a MealPlan ⇄ base64 ⇄ `#plan=`;
                    read_hash on startup, hash_url/write_hash (replaceState, no history
                    entry) after changes, share_url for the Share button.
  storage.rs        Persistence BOUNDARY. PersistedState {version, plan, grocery} ⇄ JSON ⇄
                    localStorage (decode takes today, migrating weekday plans); GroceryState
                    {checked (with amount snapshots), omitted, hide_to_taste, range:
                    GroceryRange {All | Days(n)} with until(plan, today)} + visible()/
                    remaining(); resolve_initial (hash plan, else last plan, else empty;
                    the device's grocery state always kept);
                    prune_checked (drops a tick only when the line is gone or grew).
  pwa.rs            Registers the service worker + injects the manifest <link> (prod only).
  components/
    nav.rs            Recipes / Week / Grocery tabs: bottom bar on phones, top bar ≥720px
                      (CSS); grocery badge = items left; hidden on the recipe page.
    modal.rs          Generic Modal: bottom sheet on phones, dialog on desktop (CSS);
                      optional title; closes on backdrop/Esc/button.
    toast.rs          One-line confirmation set via app::notify, auto-hides.
    stepper.rs        "− n +" control (servings).
    recipe_filters.rs Search box + chip row ("by <author>" when there are 2+ authors, tags,
                      cuisines) driving a RecipeQuery.
    recipe_picker.rs  Recipe picker sheet for a day; appends to the plan.
    add_to_week.rs    "Add to week" button + sheet of the rolling dates (recipe page).
    icons.rs          Inline SVG icons.
    mod.rs            also today() (local browser date) and rolling_dates(today, plan)
                      (14 days from today, or through the last planned day).
  pages/
    recipes_page.rs       Compact list with search/filter chips; rows link to the recipe.
    recipe_detail_page.rs Single recipe (/recipe/:id): servings stepper rescales ingredients,
                          tappable steps (not persisted), AddToWeek.
    planner_page.rs       "Plan": Share button (copies the address), the rolling dates from
                          today (cards for today and days with meals, thin rows for empty
                          days, a "next week" divider before Mondays), meal sheet
                          (servings, view, remove), clear.
    grocery_page.rs       Range chips (Today · 3 days · 7 days · All planned) and the dates
                          covered in the summary; aisles with tap-to-tick rows (ticked sink); holding a row opens its
                          sheet ("needed for …", "I have it at home"); at-home chips,
                          free-text extras with suggestions, to-taste toggle, copy.
```

### Key design boundaries

- **`recipes.rs` — the data boundary.** Everything consumes recipes/catalog through
  `load_data()` (async, `Resource` + `<Suspense>`). Switching to runtime HTTP fetch means
  rewriting only the loader bodies; pages are untouched.
- **`quantity.rs` — the unit boundary.** The only place that knows what a tablespoon is.
  Recipes keep `unit` as text (labels like "clove" need the catalog to resolve); the catalog's
  `unit` is a typed `Unit`. Conversion always targets the ingredient's shopping unit; when
  none exists the amount is kept in its own dimension rather than dropped.
- **`state.rs` / `storage.rs` — the URL and persistence boundaries.** Pure encode/decode
  cores with host tests; the `window` access is a few small wrappers. **The URL is the
  plan**: startup takes the plan from the hash, else the last plan stored on the device, else
  an empty plan, and `UrlSync` keeps the hash current with `replaceState` (no history
  entries; an empty plan drops the fragment). Opening someone's link simply shows their plan
  — the previous one is still in browser history. Grocery state is per device and never in
  the URL.
  Both payloads are untagged JSON with defaulted fields and backward-compatible decoders, so
  every link ever shared keeps decoding (weekday plans land on their next dates as of
  today, see `PlanWire`); `storage` carries a `version` (now 3) so an older
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
{ "id": "lentil_dal", "title": "Lentil Dal", "author": "Claude", "servings": 4,
  "image": "images/lentil_dal.svg",
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

**Meal plan** (in-memory, share link, localStorage) — `MealPlan { dates: BTreeMap<Date,
DayPlan>, extras }`, serialized as `{"dates": {"2026-10-27": [...]}, "extras": [...]}`. Days
without meals are not stored and past dates are dropped on decode and at midnight rollover.
Decoding goes through `PlanWire`, which also accepts the earlier `{days: [7 × DayPlan]}`
shape and maps each weekday (Monday = 0) to its next date on or after today. A `DayPlan` is a plain **list** of `MealEntry` (recipe id + optional servings) in the order
added — no fixed breakfast/lunch/dinner slots. On the wire a day is a JSON array; the decoder
also accepts the original `{breakfast, lunch, dinner}` object (slots become list entries). A
`MealEntry` serializes as a bare string when servings are default and as an object otherwise.
`extras` are `Extra`s: `Catalog(RecipeIngredient)` (the pre-existing shape, summed like recipe
lines) or `Custom {name, quantity?}` (free text, shown as typed in the "Other" aisle);
omitted when empty.

**Persisted state** (localStorage key `cooking-rs.state`; `plan` is the last plan opened) —
`{ version: 3, plan, grocery: { checked: [{key, subtotals}], omitted: [key], hide_to_taste,
range: "all" | {"days": n} } }`. The grocery list covers today through `range` (default: the
last planned day); the nav badge uses the same range. A tick remembers the amounts it was
made at and is dropped only when the line disappears or now needs more (a larger amount or a
new dimension), so a past day dropping off keeps what you bought ticked; omissions are
sticky.

**Invariants** (enforced by `src/validate.rs` under `cargo test`): every data file parses;
recipe `id` equals the file stem and is unique; every recipe has an `author` ("Claude" marks
the generated ones, which the recipe filter can then separate from your own); every ingredient key exists; every unit
resolves and converts into the ingredient's shopping unit; catalog units are base units;
every recipe image exists on disk; no step is the scaffolder's `TODO`; every catalog category
is an aisle in `shopping::CATEGORY_ORDER`.

## 6. Build, run, deploy

- **Dev:** `trunk serve --open` — compiles to wasm, serves at `/`, live-reloads on save.
  The service worker is not registered on localhost (and self-unregisters), so dev never
  serves stale assets.
- **Test:** `cargo test` — unit tests for every pure module plus the data validation suite.
- **CI:** `.github/workflows/ci.yml` runs on every push and pull request:
  `cargo fmt --all --check` → `cargo clippy --all-targets -- -D warnings` → `cargo test` →
  `trunk build` (wasm). A green CI means a deployable tree.
- **Author:** write a plain-text draft (see `src/scaffold.rs`) and run
  `cargo run --bin scaffold -- draft.txt` (`--dry-run` prints, `--force` overwrites). It
  writes `recipes/<id>.json` and a placeholder `images/<id>.svg`, or lists every problem by
  line (unknown ingredients come with a catalog stub to paste) and writes nothing.
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
  the base; `min-width: 720px` (top bar, dialogs) widens it; the plan stays one centered column at every width.

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
  with tap-to-tick and hold-for-options, free-text extras with catalog suggestions; share
  button + shared-plan preview banner; dark mode.
- **Phase 7 — Content and authoring.** Crate split into lib + app/scaffold binaries; the host
  scaffolder (draft text → validated recipe JSON + placeholder image); 12 weeknight dinners
  (25 recipes), Fish and Frozen aisles, aisle and `TODO` validation rules. A required `author`
  on every recipe ("Claude" for the generated ones) with "by …" filter chips.
- **Phase 8 — Plan by date.** The plan is keyed by calendar date (pure `date.rs`, no
  dependency); a rolling 14-day list from today on every width, empty days as thin rows,
  "next week" dividers; weekday plans and links migrate to their next dates (storage v3);
  grocery range chips (Today · 3 days · 7 days · All planned) shared with the tab badge; a
  tick survives when the amount needed shrinks; "Add to week" lists the same dates.
- **Phase 9 — The URL is the plan.** For a family sharing one plan: the plan lives in the
  `#plan=` fragment and is rewritten on every change and navigation, so the address is always
  shareable; opening a link shows that plan (the preview banner and `own_plan` are gone);
  `localStorage` keeps the last plan, for the home-screen icon, and the device's grocery state.

### Next
Each milestone ships on its own (CI gate green, deployed, checked live).

1. **Filter sheet.** Search plus a "Filters · N" button opening a sheet with Diet
   (vegetarian or meat & fish, from the `vegetarian` tag, which validation keeps honest),
   Author, Cuisine, Time (≤20 / ≤40 min) and Tags; multi-select (any author/cuisine/diet,
   all tags), option counts, removable chips for the active filters. On the recipes page the
   filters live in the query string, so they survive opening a recipe and can be shared.
2. **Meal editing.** Move a meal to another date (shared date list with "Add to week"),
   reorder within a day with up/down, and a "Clear plan" sheet with "Meals only" (keeps
   extras and at-home items) or "Everything", replacing `window.confirm`. Both clear ticks.
3. **Live with it.** A week of real planning and one shop on the phone; a fix round
   follows and re-prioritises the polish below.
4. **Grocery polish.** Tap an extra to edit it, finished aisles fold to one line,
   "Clear ticks" for the next shop.
5. **PWA and offline.** Airplane-mode check of the installed app; self-host the two
   fonts so they work offline; bump the cache.

### Later, on demand
- **Real photos** once they exist (hero on the recipe page, list thumbnail).
- **Densities** for volume-measured solids ("1 cup flour", "1 tsp cumin", "1 tbsp honey"),
  a one-arm extension of `to_shopping`. Until then, spices, honey and butter are written in
  grams.
- **Shorter share links** if they ever get unwieldy (short recipe ids, deflate).
- **Runtime-fetched recipes**: swap the `recipes.rs` loader to fetch JSON over HTTP so data
  changes don't need a rebuild. The async-first boundary already anticipates this.

"Why is this on the list" is done: the grocery row sheet shows the recipes that need it.

### Non-goals (for now)
User accounts, a backend/database, server-side rendering, nutrition tracking, in-app
recipe editing, and image hosting. The app stays a static, link-shareable SPA.

---

> Update this document when a boundary, the data model, or the deploy flow changes — it is
> the map newcomers (human or AI) read first.
