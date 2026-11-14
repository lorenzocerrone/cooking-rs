# menu.lock — rewrite plan

This is the brief for rewriting **cooking-rs** (this repo, now the proof of concept) as
**menu.lock**, a TypeScript web app in a new repository, **`menu-lock`**. It is written for
the agent that bootstraps and builds the new repo: read all of it before the first commit,
and come back to the parity checklist (§6) and the rules (§5) whenever in doubt.

The old repo stays available as the reference implementation. Attach
`lorenzocerrone/cooking-rs` to the session read-only when you need it: every Rust module
in `src/` has unit tests at the bottom that are, in effect, the spec for the logic being
ported. Where this document and the old code disagree, **this document wins** (several
behaviours are deliberately simplified, see §3).

---

## 1. What menu.lock is

A small, phone-first, static web app for a household that cooks at home:

1. **Recipes**: browse, search and filter a curated set of recipes, and cook from one.
2. **Plan**: put recipes on the coming days (a rolling two weeks).
3. **Grocery list**: everything the plan needs, summed in shopping units, grouped by
   aisle, ticked off in the shop.

**The plan is a lockfile.** The whole plan lives in the URL. Bookmarking, copying the
address bar, or pressing Share all give the same link, and opening it gives the same
plan. No accounts, no server, no database. That idea gives the app its name and its
voice: a geeky, no-nonsense tool that treats your week of dinners like a pinned
dependency file and "resolves" it into a shopping list.

### Spirit

- **No nonsense.** Every screen does one job. No onboarding, no settings page, no
  feature that needs explaining.
- **Personality through craft, not clutter.** Paper, ink, monospace numbers, motion that
  feels physical, dry terminal-flavoured copy. Jokes never replace clarity: every label
  still says what it does.
- **Easy for an agent to change.** Boring, popular tools; strict types; one command that
  checks everything; screenshots the agent can look at. This matters more than elegance.

### Non-goals (do not build)

- Adding or editing recipes inside the app (no editor, no "paste a recipe", no
  scaffolder CLI). Recipes are added by the `add-recipe` agent skill (§9).
- Accounts, backend, database, sync between devices or family members.
- Nutrition, calories, macros.
- Uploaded or photographed recipe images.
- Moving off GitHub Pages (Cloudflare can come later).
- New features beyond the parity checklist, unless the user asks.

---

## 2. Decisions already made (do not reopen without the user)

| Topic | Decision |
| --- | --- |
| Name | App: **menu.lock**. Repo and URL path: **`menu-lock`**. |
| Language | TypeScript (strict). |
| UI | React + Vite. |
| Hosting | GitHub Pages (`<user>.github.io/menu-lock/`), with per-PR preview sites. |
| Recipes | Static JSON files in the repo, added by the `add-recipe` skill. |
| Ingredients | **Closed world**: every recipe ingredient must exist in the ingredient dictionary. The skill adds missing ones to the dictionary first. |
| Grocery ticks | **One state: ticked.** No "have it at home", no "omitted", no "hide to taste" toggle. Pantry staples and to-taste items show like everything else; you tick them or ignore them. |
| Images | Keep the 25 existing SVG illustrations. New recipes get a shared placeholder. A tag-based illustration set is a **later** session. |
| Look | Keep the parchment / ink / sienna paper style and Space Grotesk + Space Mono, and evolve it freely. |
| Legacy links | Only the current dated plan format is read. Older shapes (weekday plans, breakfast/lunch/dinner slots, `ingredient` field) are dropped. |
| Old site | Stays up until menu.lock reaches parity, then redirects to it, carrying the plan (§10). |

---

## 3. What changes from cooking-rs, and why

The old app works; these are the deliberate differences.

1. **No in-app recipe authoring.** `editor.rs`, `paste.rs`, `scaffold.rs`, the `/new`
   page, the `scaffold` and `ingredients` binaries: not ported (~1,500 lines).
2. **Closed-world ingredients.** The old app accepted free ingredient names (for the
   editor) and had to handle unknown names: custom grocery lines, a per-line `aisle`
   field, free piece words like "bunch", mixed-unit subtotals ("2 pcs + 100 g"), an
   unknown-ingredient report. All of that goes. Recipes reference dictionary **keys**,
   and validation requires every amount to **convert into the ingredient's shopping
   unit**. Consequence: every recipe-derived grocery line is exactly **one number in one
   unit** (plus an optional "to taste"). The current data already satisfies this (the old
   `cargo run --bin ingredients` reports "Every ingredient matches the dictionary and
   adds up").
3. **Derived, not written.** `vegetarian` is computed (no ingredient from the Meat or
   Fish aisle), not a tag. Total time is computed. Filter options are computed from the
   data.
4. **Resolve once.** On load, each recipe line is resolved to
   `{ key, perServing: number | null, optional }` in the shopping unit. The grocery list
   then only multiplies and adds. All unit rules live in one module with its tests.
5. **One schema.** Zod schemas are the single definition of the data files: TypeScript
   types, validation in tests, and JSON Schema for editor autocomplete are all generated
   from them. The old code repeated this three times.
6. **One grocery state.** Ticks only, plus the day range.
7. **Grocery rows: tap to tick, nothing else.** The old "hold a row for a sheet" gesture
   goes. What it showed ("Needed for Carbonara, Dal") is shown inline under the row.
8. **Hash routing.** URLs look like `/menu-lock/#/planner?plan=…`. On GitHub Pages this
   removes the 404.html fallback script, the deep-link redirect hack for previews, and
   all base-path bugs in links. The only base path left is Vite's asset `base`.
9. **Self-hosted fonts.** The old app loads Google Fonts from the network, so offline it
   falls back to system fonts. Bundle them.
10. **Generated service worker** (vite-plugin-pwa) instead of the hand-written `sw.js`.
11. **Short docs.** A short `CLAUDE.md` and a `docs/decisions.md` log, instead of a 30 KB
    architecture file that restates the code.

---

## 4. Stack and project layout

Use current stable versions at bootstrap time and check each library's current docs
(APIs move: e.g. Tailwind v4 is configured in CSS, not `tailwind.config.js`).

| Concern | Choice | Notes |
| --- | --- | --- |
| Build | Vite | `base` from env (`/menu-lock/`, or the preview path). |
| UI | React + TypeScript (strict, `noUncheckedIndexedAccess`) | |
| Routing | TanStack Router, **hash history** | Typed, Zod-validated search params for `plan` and the recipe filters. Keep `plan` on every route (its search-param retention middleware, or equivalent). |
| State | Zustand | One store: plan + grocery state. Persist to localStorage; sync plan to URL. |
| Styling | Tailwind CSS (v4) + CSS variables for the design tokens | Light and dark themes from the same tokens. |
| Motion | Motion (`motion/react`) | Layout animations, presence, springs, shared-element transitions. |
| Sheets | Vaul | Draggable bottom sheets on phones; dialog on wide screens. |
| Drag & drop | dnd-kit | Reorder meals within a day and drag them between days. |
| Data validation | Zod | Data files, plan wire format, stored state. |
| Lint + format | Biome | One tool. |
| Unit tests | Vitest | All pure logic + data validation. |
| Browser tests | Playwright | Core flows + screenshots. Cloud sessions have Chromium pre-installed: do not run `playwright install`. |
| PWA | vite-plugin-pwa | Precache everything; offline-first; prompt or auto-update. |
| Fonts | `@fontsource/space-grotesk`, `@fontsource/space-mono` | |
| Dates | A tiny own module over ISO `YYYY-MM-DD` strings | No date library needed (see §5.6). |

Keep the dependency list to this. Ask before adding a component library.

### Layout

```
data/
  ingredients.json          ingredient dictionary (§5.1)
  recipes/<id>.json         one recipe per file (§5.2)
public/
  images/<id>.svg           recipe illustrations (+ placeholder.svg)
  icons/                    PWA icons (regenerate for menu.lock)
src/
  lib/                      PURE logic: no React, no DOM, no window (enforce with a lint rule)
    schema.ts               Zod schemas for data files; generated JSON Schema
    data.ts                 load + validate + resolve recipes and dictionary
    units.ts                units, conversion into shopping units
    dates.ts                ISO-date helpers; "today" is always a parameter
    plan.ts                 plan operations (add, remove, move, reorder, servings, clear, drop past)
    planCodec.ts            plan ⇄ JSON ⇄ base64url (§5.4)
    grocery.ts              aggregation, ticks, pruning, range (§5.5)
    filters.ts              recipe query, facets, option counts, query-string codec (§6.1)
    extras.ts               parse "milk 1 l", suggestions, to-extra
    format.ts               every user-facing number/date/quantity string
  state/
    store.ts                Zustand store (plan, grocery), actions call lib/
    storage.ts              localStorage boundary (§5.7)
    urlSync.ts              store ⇄ ?plan=
  ui/                       reusable components (sheets, chips, stepper, toast, icons…)
  routes/                   pages: recipes, recipe/$id, planner, grocery
  styles/                   tokens.css, base.css
tests/e2e/                  Playwright flows
scripts/
  shots.ts                  screenshots of every screen (phone + desktop, light + dark)
  migrate-from-cooking-rs.ts  one-off data migration (§8, phase 2)
.claude/skills/add-recipe/  the recipe skill (§9)
CLAUDE.md                   short agent guide
docs/decisions.md           decision log (start by copying §2 and §3)
```

### Commands (package.json scripts)

- `npm run dev`: dev server.
- `npm run check`: Biome, `tsc --noEmit`, Vitest (logic + data validation). **The one
  command to run before every commit.** It must be fast (seconds).
- `npm run e2e`: Playwright flows (pass `--trace off` unless debugging).
- `npm run shots`: build + preview + screenshot every screen to `shots/` (gitignored), at
  390×844 (phone) and 1280×800, light and dark. Use it after any visual change and look
  at the images.
- `npm run build`: production build into `dist/`.

---

## 5. Rules carried over (the parts that must be exactly right)

### 5.1 Ingredient dictionary (`data/ingredients.json`)

```json
{
  "onion":    { "name": "Onion",    "unit": "g",   "aisle": "Produce", "count": { "perPiece": 150 } },
  "garlic":   { "name": "Garlic",   "unit": "g",   "aisle": "Produce", "count": { "label": "clove", "perPiece": 4 } },
  "chopped_tomatoes": { "name": "Chopped tomatoes", "unit": "pcs", "aisle": "Pantry",
                "count": { "label": "can", "perPiece": 400 } },
  "egg":      { "name": "Egg",      "unit": "pcs", "aisle": "Dairy & Eggs" },
  "scallion": { "name": "Scallion", "unit": "g",   "aisle": "Produce", "count": { "perPiece": 15 },
                "aliases": ["spring onion", "green onion"] }
}
```

- Key: `snake_case`, stable forever (it is the grocery tick identity and appears in
  share links).
- `unit` is the **shopping unit**: how it is bought and summed. Only `g`, `ml` or `pcs`.
- `aisle` is one of, in this display order: `Produce`, `Meat`, `Fish`, `Dairy & Eggs`,
  `Pasta & Grains`, `Bakery`, `Legumes`, `Pantry`, `Frozen`, `Spices & Oils`. Hand-typed
  extras that are not in the dictionary go to `Other`, always last.
- `count` (optional) says how the ingredient is counted in pieces:
  - `perPiece`: one piece expressed in the shopping unit. For a `g` ingredient, grams per
    piece; for a `pcs` ingredient, the grams in one piece (so "400 g chopped tomatoes"
    becomes 1 can).
  - `label` (optional): the piece word recipes may use ("clove", "can", "fillet",
    "sprig", "thumb"). `plural` (optional) defaults to label + "s".
- `aliases` (optional): other names for the same thing. Used by the extras search, the
  `add-recipe` skill, and to read catalog extras in old share links.
- No two entries may share a name or alias (case-insensitive, singular/plural folded).

### 5.2 Recipes (`data/recipes/<id>.json`)

```json
{
  "id": "lentil_dal",
  "title": "Lentil Dal",
  "author": "Claude",
  "servings": 4,
  "description": "Red lentils simmered with onion, garlic and coconut milk.",
  "cuisine": "Indian",
  "tags": ["dinner", "comfort"],
  "prepTime": 10,
  "cookTime": 30,
  "image": "lentil_dal.svg",
  "ingredients": [
    { "ingredient": "lentils", "amount": 300, "unit": "g" },
    { "ingredient": "onion", "amount": 1, "note": "finely chopped" },
    { "ingredient": "garlic", "amount": 3, "unit": "cloves" },
    { "ingredient": "olive_oil", "amount": 1, "unit": "tbsp" },
    { "ingredient": "chives", "amount": 5, "unit": "g", "optional": true },
    { "ingredient": "salt" }
  ],
  "steps": ["Soften the onion and garlic in the oil.", "…"]
}
```

- `id` equals the file name and never changes once published (it is in share links).
- Required: `id`, `title`, `author`, `servings` (≥ 1), `ingredients` (≥ 1), `steps` (≥ 1).
  Optional: `description`, `cuisine`, `tags`, `prepTime`, `cookTime` (minutes), `image`
  (file in `public/images/`; absent means the shared placeholder).
- Unknown fields are errors (catches typos).
- An ingredient line: `ingredient` (dictionary key), `amount` (> 0; absent = "to taste"),
  `unit`, `note` (shown after the name), `optional`.
- `unit` may be `g kg ml l tsp tbsp cup pcs` (also the long spellings: gram(s),
  kilogram(s), milliliter/millilitre(s), liter/litre(s), teaspoon(s), tablespoon(s),
  cup(s), piece(s), pc), or the ingredient's piece `label` / plural. **No unit means a
  count of pieces** ("onion, 1"), whatever the dictionary's shopping unit.
- No `unit` without an `amount`.
- The tag `vegetarian` is not allowed: it is computed (§6.1).
- **Every amount must convert into the ingredient's shopping unit** (§5.3); a line that
  doesn't is a validation error.
- The recipe page shows the dictionary `name`, the amount in the unit **as written** in
  the recipe (scaled by servings), and the `note`.

### 5.3 Units and conversion

- Dimensions: mass (`g`, `kg` = 1000 g), volume (`ml`, `l` = 1000 ml, `tsp` = 5 ml,
  `tbsp` = 15 ml, `cup` = 240 ml), count (`pcs`, piece labels).
- Into the shopping unit:
  - same dimension: convert to the base unit (g, ml, pcs);
  - count → mass or volume: pieces × `perPiece`;
  - mass → count (a `pcs` ingredient): grams ÷ `perPiece`;
  - anything else (e.g. tbsp of a `g` ingredient, pieces without `perPiece`): **not
    convertible, so the recipe fails validation**. Fix it in the data (change the unit, or
    add `count` to the dictionary entry). There is no density table; if one is ever
    needed, ask the user.

### 5.4 The plan and its URL (the lockfile)

In memory:

```ts
type MealEntry = { recipe: string; servings?: number }   // servings absent = recipe default
type Extra =
  | { kind: "catalog"; key: string; amount?: number; unit?: string }
  | { kind: "custom"; name: string; quantity: string | null }
type Plan = { dates: Record<IsoDate, MealEntry[]>; extras: Extra[] }
```

Rules:
- A day is an ordered list of meals (no breakfast/lunch/dinner slots). Days without meals
  are never stored.
- Days before today are dropped on decode and whenever "today" changes (§5.6).
- Setting servings equal to the recipe default stores "absent" (keeps links short).

**Wire format: must stay byte-compatible with cooking-rs links.** JSON, then base64url
without padding:

```json
{"dates":{"2026-11-16":["carbonara",{"recipe":"lentil_dal","servings":4}]},
 "extras":[{"name":"milk","amount":1,"unit":"l"},{"name":"Toilet paper","quantity":"1 pack"}]}
```

- A meal is a bare string (default servings) or `{recipe, servings}`.
- `dates` and `extras` are omitted when empty. An empty plan means no `plan` param at all.
- Extras are untagged on the wire. **Custom** extras always carry `quantity` (string or
  `null`); **catalog** extras never do and have `name` (+ optional `amount`, `unit`,
  `note`, `optional`). On encode, write the dictionary key as `name`. On decode, look the
  `name` up in the dictionary by key, display name or alias (case-insensitive,
  singular/plural folded). If it isn't found, read it as a custom extra with that name and
  the amount + unit as its quantity text.
- Unknown recipe ids in a link are kept (not dropped), and shown as
  "Unknown recipe (id)", so a link from a newer version doesn't lose data.
- Garbage (bad base64, bad JSON, wrong shape) decodes to "no plan in the URL".

Location: `?plan=<base64url>` in the hash-route search params, e.g.
`/menu-lock/#/grocery?plan=eyJkYXRlcyI6…`. Every route carries it. After each plan change,
rewrite the URL with `replace` (no history entry). The Share button copies the current
address.

### 5.5 Grocery list

**Aggregation**, for the meals dated today through the range end, plus all extras:
- For each meal: factor = chosen servings ÷ recipe servings. Each resolved line adds
  `perServing × chosen servings` (= amount × factor) to the line for its dictionary key.
- A line with no amount marks the grocery line "to taste" (also in the text export).
- A grocery line is optional only if **every** contribution is optional.
- Sources: the recipe titles that contributed, de-duplicated, in plan order.
- Catalog extras add like recipe lines (unscaled, no source; shown as "extra").
- Custom extras: if their name matches a dictionary entry that already has a line, their
  quantity text is appended to that line ("+ 1 loaf"); otherwise they form their own line
  in `Other`, merged by case-insensitive name, quantities listed as typed.
- Aisles in the order of §5.1, then any unknown aisle alphabetically, `Other` last.
  Lines sorted by name, then key. Within an aisle, unticked lines first, ticked lines
  sink to the bottom.

**Display rounding** (the shop-friendly numbers):
- Mass: below 10 g, one decimal; below 1000 g, whole grams; from 1000 g, kg with up to
  two decimals ("1.25 kg"). Volume the same with ml / l. Trailing zeros dropped.
- Pieces: always rounded **up** to whole pieces (1.2 onions → 2), labelled with the
  piece word if any ("3 cloves", "1 can", "2 cans"), otherwise a bare number ("4").
- Recipe page amounts: the authored unit, at most two decimals ("0.5 tbsp", "3 cloves").

**Range** (per device, not shared): presets **Today**, **3 days**, **7 days**,
**All planned** (default). "All planned" means today through the last planned day.
A summary line shows "N of M left to buy · <range> · <K meals>". The range label is
"Today", "Today – Tomorrow", or "Sun 25 – Sat 31" style.

**Ticks** (per device, not shared): tapping a row toggles it. A tick records the line's
amount at the time. When the plan changes, a tick is **dropped** if its line disappeared
or now needs more than when ticked (a larger amount, or a custom quantity it didn't
have). A line that needs less (e.g. a past day dropped off) stays ticked. Compare
amounts rounded to 3 decimals so float noise never unticks.

**Badge**: the Grocery tab shows the number of unticked lines in the current range.

**Copy list**: plain text, aisles as headings, "- Name: quantity" lines (no colon when
there is no quantity), a blank line between aisles, every line in the range (ticked or
not, as in cooking-rs). With nothing to buy: "No meals selected yet."

### 5.6 Dates

- Plans are keyed by local calendar date, ISO `YYYY-MM-DD`. "Today" comes from the
  device's local clock, read in the UI layer and passed into the logic (logic never
  reads the clock: easy to test).
- When the page becomes visible again (`visibilitychange`) and the date has changed,
  update "today": past days drop off the plan.
- The planner shows 14 days from today, or through the last planned day if later.
- Labels: "Today", "Tomorrow", otherwise "Mon 27 Oct". Mid-sentence: "today",
  "tomorrow", "Mon 27 Oct" ("Added to tomorrow").

### 5.7 Local storage

- Key `menu-lock.state`, JSON `{ version: 1, plan: <wire JSON>, grocery: { checked:
  [{ key, amount, custom }], range: "all" | { days: n } } }`. Refuse a payload with a
  newer `version` than the build knows. Writes ignore failures (private mode, quota).
- **Startup precedence:** a plan in the URL wins; else the last plan stored on this
  device; else empty. The grocery state is always this device's.
- PR previews use their own key (`menu-lock.pr-12.state`) so they never touch the real
  data.
- **One-time import from cooking-rs:** both apps live on the same origin
  (`<user>.github.io`), so localStorage is shared. On first start, if `menu-lock.state`
  is absent and `cooking-rs.state` exists, import its `plan` only (decode it with the
  wire rules above; ignore its grocery state). This keeps the home-screen icon's plan
  working after the redirect.

---

## 6. Parity checklist (behaviour to keep)

This is what "the same app" means. Each item needs a test (unit or e2e) before Phase 3
is done.

### 6.1 Recipes (`#/`)

- [ ] Every recipe, with search (title + description, case-insensitive) and filters.
- [ ] Filters, in a sheet ("Filters · N" button), with the active ones shown as
      removable chips:
  - **Diet**: veg / meat & fish (any-of). Computed: a recipe is vegetarian when none of
    its ingredients is in the Meat or Fish aisle.
  - **Author**: "by X" (any-of), shown only when there are 2+ authors.
  - **Cuisine** (any-of).
  - **Time**: ≤20 min, ≤40 min (prep + cook, one at a time).
  - **Tags** (all-of).
  - Sections combine with AND. Each option shows how many recipes it would give: for an
    any-of section the option replaces that section's selection; a tag is added to the
    selected tags. Options giving 0 are dimmed.
  - Sheet footer: "Clear" and "Show N recipes" ("Show 1 recipe").
- [ ] The query is in the URL (`q`, `diet=veg,meat`, `by`, `cuisine`, `tag` comma
      lists, `time=20`), so it survives opening a recipe and coming back, and a filtered
      list can be sent.
- [ ] Each recipe shows its illustration, title, total time, cuisine, servings, tags.

### 6.2 Recipe (`#/recipe/<id>`)

- [ ] Image, title, description, cuisine, "10 min prep + 15 min cook", tags.
- [ ] Servings stepper (− n +, minimum 1) rescales every amount live; "to taste" never
      scales.
- [ ] Ingredients: dictionary name, scaled amount in the authored unit, note, optional
      flag.
- [ ] Steps, numbered; tapping a step marks it done (not persisted).
- [ ] "Add to week" opens a day picker: the rolling days with their meal counts; picking
      one appends the recipe and toasts "Added to tomorrow" (etc.).
- [ ] Back button; the tab bar is hidden on this page.
- [ ] Unknown id: a friendly not-found with a way back.

### 6.3 Plan (`#/planner`)

- [ ] Rolling dates (§5.6). Today and days with meals are cards; other days are compact
      rows that open the recipe picker. A "next week" divider before each Monday after
      today.
- [ ] Each meal shows its title and, if custom, its servings.
- [ ] "Add a meal": the recipe picker (search + the same filters, inline, not a sheet on
      a sheet); tapping a recipe appends it to that day.
- [ ] Tapping a meal opens its sheet: servings stepper; move up / move down within the
      day; "Move to…" (day picker without its own day; toast "Moved to Fri 31 Oct");
      "View recipe"; "Remove".
- [ ] **New:** drag a meal to reorder within a day or drop it on another day (the sheet
      actions stay as the accessible path).
- [ ] "Share": copies the address; toast "Link copied".
- [ ] "Clear plan" (disabled when empty) opens a sheet: **Meals only** (keeps extras) or
      **Everything** (meals and extras). Both clear the grocery ticks.

### 6.4 Grocery (`#/grocery`)

- [ ] Range chips, summary line, list by aisle, rules of §5.5.
- [ ] Row: tap toggles the tick (large tap target). Shows name, quantity, "to taste",
      "(optional)", and in small text what it's for ("carbonara, dal", or "extra").
- [ ] Empty state with a link to the plan.
- [ ] **Extras**: one input, "milk 1 l". The first number starts the quantity; the token
      after it joins the quantity unless that leaves no name ("2 eggs" → eggs, "2");
      decimal commas accepted. While typing, dictionary names and aliases are suggested
      (starts-with first, then contains, each by name). Picking one makes a catalog extra
      **if the quantity converts** for it (§5.3); otherwise it is kept as a custom extra
      exactly as typed ("1 loaf" of bread sold by weight). Enter without a pick adds a
      custom extra (aisle `Other`). Extras are listed with a remove button, and they
      travel with the share link.
- [ ] "Copy list" (§5.5), toast "List copied".

### 6.5 App-wide

- [ ] Tabs: Recipes / Plan / Grocery. Bottom bar on phones, top bar from 720 px. Grocery
      badge (§5.5).
- [ ] Toasts for confirmations.
- [ ] Sheets: bottom sheets on phones (drag to dismiss, snap points where useful),
      centred dialogs on wide screens; close on backdrop, Esc, and a close button.
- [ ] Light and dark themes following the system.
- [ ] Installable PWA, works offline after the first visit (fonts included).
- [ ] `prefers-reduced-motion`: animations reduced to simple fades or none.
- [ ] Accessibility: 44 px tap targets, visible focus, AA contrast in both themes,
      sheets and drag-and-drop usable from the keyboard.

---

## 7. Design direction: evolve the paper

Start from the current tokens (light / dark):

| Token | Light | Dark |
| --- | --- | --- |
| paper | `#efe8d8` | `#221d16` |
| panel | `#f8f3e7` | `#2c261d` |
| inset | `#fbf8ef` | `#332c22` |
| ink | `#3b3327` | `#ede4d0` |
| muted | `#7c6e54` | `#b5a68a` |
| faint | `#9e8b68` | `#8a7d64` |
| sienna (accent) | `#a85e30` | `#d18653` |
| danger | `#9b3b2e` | `#e08a7a` |

Plus Space Grotesk for text, Space Mono for every number and label, 6 px corners,
hairline rules. Keep this family; push it further. Ideas to explore (pick with the user
in Phase 1, not all at once):

**Texture and depth**
- Faint paper grain, ruled or dotted-grid backgrounds, torn or perforated edges on the
  grocery list (it's a receipt).
- Layered soft shadows only on things you can lift (cards being dragged, sheets).
- Hand-drawn sienna underlines and circled numbers.

**The lockfile motif**
- The planner reads like a file: a header `menu.lock · 14 days · 9 meals`, mono line
  numbers or dates in a gutter, a "pinned" marker on planned days.
- Share is "copy the lockfile", the grocery list is "resolved from menu.lock".
- Empty states in a dry terminal voice: `menu.lock: nothing pinned yet`,
  `0 to buy. Suspicious.`
- Optional: a small "modified since you shared" dot next to Share.

**Motion** (Motion library; transform and opacity only; 150–300 ms; springs for
anything dragged)
- Recipe cards: filtering animates cards out and closes the gaps; the filter count rolls.
- Card → recipe page: the illustration grows into the page header (shared element) and
  shrinks back.
- Recipe page: a header that collapses as you scroll; amounts animate when servings
  change; a done step strikes through with a drawn line.
- "Add to week": a token flies to the Plan tab and the badge bounces.
- Planner: meals lift and drop with springs; days slide in and out as dates roll; meals
  animate in and out when added or removed.
- Grocery: the tick draws itself, the row strikes through and slides to the bottom of
  its aisle; per-aisle progress fills; a small celebration when everything is ticked.
- Page transitions: short slide or crossfade between tabs; animated tab icons.
- Haptics (`navigator.vibrate`) on tick and drop where supported.

**Illustrations**: keep the 25 SVGs, give them a consistent frame (paper card, slight
rotation, tape corners…). New recipes use `placeholder.svg` in the same frame. A
tag-based set comes later.

**Rules**: clarity beats cleverness; every animation must survive reduced motion; test
on a phone-sized viewport first; look at `npm run shots` output after every visual change.

---

## 8. Phases

Work in this order. Each phase ends with `npm run check` green, a short update to
`docs/decisions.md`, and a message to the user with what changed (screenshots for UI
phases). Commit in small steps (`feat(scope): …`, `fix(scope): …` style, as in
cooking-rs).

### Phase 0: Bootstrap

- Vite + React + TS strict, Biome, Vitest, Playwright, Tailwind, TanStack Router (hash
  history), Zustand, Motion, Vaul, dnd-kit, Zod, fontsource fonts.
- `npm run check / e2e / shots / build` working on a hello-world page.
- CI workflow on every push and PR: `npm ci`, `check`, `build`, `e2e`.
- Deploy workflow: on push to `main`, build with `base=/menu-lock/` and publish `dist/`
  to the `gh-pages` branch (keep the `pr-preview/` folder).
- PR preview workflow: build with `base=/menu-lock/pr-preview/pr-<n>/` and a preview
  flag (for the storage key), publish with `rossjrw/pr-preview-action` into
  `pr-preview/` on `gh-pages`, remove on close. With hash routing no 404 fallback is
  needed.
- `CLAUDE.md` (goals, commands, the rules of §5 in brief, "logic in `src/lib` stays pure",
  "run check before commit, shots after visual changes") and `docs/decisions.md`.
- **User action:** in the repo's settings, Pages → deploy from branch `gh-pages`.
- Exit: the empty app is live at `<user>.github.io/menu-lock/`.

### Phase 1: Design direction

- Build 2–3 directions for the evolved paper look as real screens with fixture data
  (recipes list, planner, grocery list; phone size; light and dark), under a dev-only
  route such as `#/lab`.
- Include one or two signature interactions per direction (e.g. tick + strike + slide;
  card-to-page transition) so the feel can be judged, not just the colours.
- Show the user screenshots and the preview link; let them pick and mix.
- Exit: chosen tokens in `src/styles/tokens.css`, a short "design rules" section in
  `docs/decisions.md`.

### Phase 2: Data and logic (no UI)

- `scripts/migrate-from-cooking-rs.ts`: read cooking-rs's `ingredients.json` and
  `recipes/*.json` (from the attached old repo), write the new format:
  - `category` → `aisle`, `per_piece` → `perPiece`, `prep_time` / `cook_time` →
    `prepTime` / `cookTime`, `image: "images/x.svg"` → `"x.svg"`;
  - each line's `name` → the dictionary key, using the old lookup (key, display name or
    alias; case-insensitive; trying the name with "es"/"s" stripped and with "s" added).
    Fail loudly on anything unmatched;
  - drop the `vegetarian` tag, but first check that the computed value agrees with it
    for every recipe (report any mismatch to the user);
  - drop the `aisle` field from lines (closed world).
  Copy `images/*.svg` to `public/images/`. Run once, commit the data, keep the script
  for reference.
- `src/lib/*` with Vitest tests. Port the Rust tests case by case from cooking-rs
  (`quantity.rs`, `models.rs`, `shopping.rs`, `format.rs`, `filter.rs`, `extras.rs`,
  `state.rs`, `storage.rs`, `date.rs`), skipping the ones about dropped behaviour (§3).
- Data validation test: every file parses with Zod; ids match file names and are
  unique; every ingredient key exists; every amount converts; no `unit` without
  `amount`; images exist; dictionary keys/names/aliases are unique; `perPiece` > 0.
  Error messages must name the file and the line, in plain words (the `add-recipe`
  skill reads them).
- Codec tests with real links: encode a plan in cooking-rs (or hand-write the base64url
  of the JSON in §5.4) and check that menu.lock decodes it to the same plan.
- Exit: all logic tests green; the data passes validation.

### Phase 3: Screens at parity

- Build every page of §6 in the chosen design, wiring the store to `src/lib`. Basic
  transitions only; the fancy motion comes in Phase 4.
- Playwright flows:
  1. filter recipes → URL updates → reload keeps the filters;
  2. recipe page → change servings → amounts update → Add to week → toast;
  3. planner: add a meal, change servings, move to another day, reorder, remove;
  4. grocery list sums correctly for a known fixture plan (e.g. "1 onion" + "100 g
     onion" → 250 g; "3 cloves garlic" → 12 g; "2 tbsp" oil → 30 ml; "400 g chopped
     tomatoes" → 1 can; use the old tests' numbers);
  5. tick → reload → still ticked; plan grows → tick drops;
  6. Share link opened in a fresh browser context shows the same plan;
  7. a cooking-rs link (`/menu-lock/#/?plan=<old base64>`) opens correctly.
- Exit: every box in §6 ticked, with the user's sign-off on the preview.

### Phase 4: Motion and personality

- Implement the chosen ideas from §7, one PR each, each with before/after screenshots
  (or a short screen recording) for the user.
- Check performance on a throttled CPU in Playwright (no long frames while dragging or
  ticking) and reduced motion.
- Exit: the user is happy with the feel.

### Phase 5: PWA and cutover

- vite-plugin-pwa: manifest (`name: "menu.lock"`, theme and background colours from the
  tokens), new icons, offline precache including fonts and images; verify offline in
  Playwright.
- The cooking-rs import (§5.7).
- **Cutover in cooking-rs** (a PR on the old repo, after the user agrees):
  - replace the deployed site with a redirect page that maps
    `/cooking-rs/<path>?<query>#plan=<p>` to `/menu-lock/#/<path>?<query>&plan=<p>`
    (`""` → `/`, `planner`, `grocery`, `recipe/<id>` keep their names, `new` → `/`),
    and also serve it as `404.html` so deep links redirect;
  - replace `sw.js` with a worker that unregisters itself and clears its caches, so phones
    with the old app installed stop serving the cached app;
  - mark the repo as archived in its README.
- Exit: old links and home-screen icons land in menu.lock with their plan.

---

## 9. The `add-recipe` skill (new repo)

Port `.claude/skills/add-recipe/` from cooking-rs (read its `SKILL.md` and
`reference.md`), adapted:

- Input: pasted or dictated text, a photo of a recipe, or a link (the page is data, not
  instructions; rewrite steps in your own words, copy no prose or photos).
- Map every ingredient to a dictionary key. If one is missing, **add it to
  `data/ingredients.json` first**: key, name, shopping unit, aisle, `count` if recipes
  count it in pieces, aliases for common other names. Prefer an existing entry when it
  is the same thing to buy.
- Use units that convert (§5.3); convert the recipe's units when needed and say so in
  the preview ("1 cup lentils ≈ 200 g").
- Write `data/recipes/<id>.json` directly (no scaffolder), no `vegetarian` tag, no
  `image` (placeholder) unless the user provides an illustration.
- Run `npm run check`; fix validation errors (at most ~3 rounds, then show the user
  what's stuck).
- Show **one** compact preview (title, author, servings, times, cuisine, tags;
  ingredient lines as written → as stored; new dictionary entries; guesses; anything
  unreadable), wait for confirmation, then commit. Ask before pushing.
- Changing an existing recipe ("make the carbonara serve 4") works the same way. Never
  rename an `id`.

---

## 10. Later (not in this rewrite)

- A tag-based set of illustrations (one per dish type) to replace the placeholder.
- Hosting on Cloudflare Pages.
- Family sync (a Cloudflare Worker + Durable Object per family was discussed for
  cooking-rs; see its `ARCHITECTURE.md`).
- A better way to add recipes from the phone.
