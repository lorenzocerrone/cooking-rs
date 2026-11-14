# add-recipe reference

Facts from the code (`src/scaffold.rs`, `src/quantity.rs`, `src/shopping.rs`,
`src/validate.rs`). If something here disagrees with the code, the code wins.

## Draft format

```text
Lentil Dal                      ← line 1: title (the id is its slug: lentil_dal)
author: Lorenzo                 ← required
servings: 4                     ← default 2
cuisine: Indian
description: Red lentils simmered with onion, garlic and coconut milk.
tags: dinner, quick             ← comma-separated, lowercased
prep: 10                        ← minutes ("10", "10 min")
cook: 30
                                ← blank line ends the headers (optional, but write it)
300 g lentils
1 onion, finely chopped         ← text after the first comma is a note
3 cloves garlic                 ← piece label of the dictionary entry
1/2 can coconut milk            ← fractions and decimal commas work
chives (optional)               ← "(optional)" at the end
salt                            ← no amount = to taste

Steps                           ← or "Method"
1. Soften the onion and garlic. ← "1." / "1)" / "-" numbering is stripped
```

Headers allowed: `id` (override the slug), `author`, `servings`/`serves`, `cuisine`,
`description`, `tags`, `prep`/`prep_time`, `cook`/`cook_time`. Anything else is an error.
No steps → a `TODO` step that `cargo test` rejects, so always write steps.
The scaffolder removes the `vegetarian` tag when an ingredient is meat or fish, and adds
it when the dictionary knows every ingredient and none is. With an ingredient it
doesn't know, put `vegetarian` in `tags` yourself when it is true.

## Amounts and units

- A leading number is the amount; the next word is the unit if it is a unit spelling
  (`g kg ml l tsp tbsp cup pcs`, also `grams`, `tablespoons`, `pieces`…), the
  dictionary entry's piece label (`clove(s)`, `can(s)`, `sprig(s)`, `thumb(s)`,
  `fillet(s)`), or a common piece word (`bunch`, `sprig`, `pinch`, `handful`, `slice`,
  `tin`…) before a name the dictionary doesn't know. Otherwise it's part of the name:
  `1 red onion` is one "Red onion".
- A bare number counts pieces: `2 onion` = two onions. No unit in the JSON always means a
  count.
- **Prefer units that convert into the entry's shopping unit**, so the grocery list adds
  them up. Mass (g, kg) ↔ mass, volume (ml, l, tsp, tbsp, cup) ↔ volume, pieces ↔
  anything via `per_piece`. There are no densities: `1 tbsp butter` is shown beside the
  grams, not added (the scaffolder warns). Convert yourself and show it in the preview
  mapping. Rough values:
  - ground spice 1 tsp ≈ 2 g, 1 tbsp ≈ 6 g; dried herbs 1 tsp ≈ 1 g
  - salt 1 tsp ≈ 6 g; sugar 1 tbsp ≈ 12 g, 1 cup ≈ 200 g; flour 1 cup ≈ 125 g
  - butter 1 tbsp ≈ 14 g, a knob ≈ 15 g; honey 1 tbsp ≈ 21 g; peanut butter 1 tbsp ≈ 16 g
  - rice 1 cup ≈ 190 g; oats 1 cup ≈ 90 g; grated cheese 1 cup ≈ 100 g
  - lentils / dried beans 1 cup ≈ 200 g; frozen peas 1 cup ≈ 140 g
  - US/imperial: 1 oz = 28 g, 1 lb = 450 g, 1 fl oz = 30 ml, 1 pint = 470 ml (US)
- The scaffolder strips nothing before the name: "a knob of butter", "juice of half a
  lemon" would become odd names. Rewrite every line yourself into
  `amount unit name, note`: "juice of half a lemon" → `0.5 lemon, juiced`.
- Vague amounts with a usual size get one, listed under Guesses: a knob ≈ 15 g, a handful
  of leaves ≈ 30 g. Those without ("a pinch", "a splash", "to taste", "for frying") →
  no amount.
- Keep the recipe's own servings; don't rescale on add.

## Mapping to the dictionary

- A name matches an entry by its key, display name or one of its `aliases`, ignoring
  case and extra spaces, singular or plural — and nothing else. A match gives the
  grocery aisle, unit conversions and merging with other recipes.
- Use the dictionary's name whenever the shopping is the same: "spring onions" →
  `scallion` (an alias, so either works); "tinned tomatoes" → `chopped tomatoes` (unit
  `can`); "extra-virgin olive oil" → `olive oil`. Put a meaningful variety in the note
  (`300 g lentils, red`).
- A different thing to buy keeps its own name ("curry leaves", "star anise"). Set
  `aisle` on the JSON line when it is clear (one of the aisles below); else it is "Other".
- `cargo run --bin ingredients` lists every name the dictionary doesn't know (with near
  matches) and every amount the list can't add up.

## Dictionary (`ingredients.json`)

Tidying it is its own task, not part of adding a recipe, and only when the user asks:
add an `aliases` entry for a synonym, or a new entry for an ingredient several recipes
use; `cargo run --bin ingredients` is the to-do list.

```json
"coconut_milk":     { "name": "Coconut milk",     "unit": "ml",  "category": "Pantry", "count": { "label": "can", "per_piece": 400 } },
"scallion":         { "name": "Scallion",         "unit": "g",   "category": "Produce", "count": { "per_piece": 15 }, "aliases": ["spring onion", "green onion"] },
```

- key: lowercase `a-z 0-9 _`, singular unless the thing is plural (`chickpeas`, `peas`).
- `name`: shown on the grocery list, capitalised ("Curry leaves").
- `unit`: the shopping unit, one of `g`, `ml`, `pcs`. Spices, honey and butter in `g`.
- `category`: exactly one aisle, in this order:
  `Produce, Meat, Fish, Dairy & Eggs, Pasta & Grains, Bakery, Legumes, Pantry, Frozen,
  Spices & Oils`. Meat and Fish make a recipe non-vegetarian.
- `count` (optional): `per_piece` = grams/ml of one piece (onion 150, garlic clove 4,
  can 400); `label` = what one piece is called; `plural` only if not label + "s".
- `aliases` (optional): other names; each name may point at one entry only (tested).

## Recipe JSON house style (for edits)

```json
{
  "id": "carbonara",
  "title": "Spaghetti Carbonara",
  "author": "Claude",
  "servings": 2,
  "image": "images/carbonara.svg",
  "description": "…",
  "cuisine": "Italian",
  "tags": ["pasta", "quick", "classic"],
  "prep_time": 10,
  "cook_time": 15,
  "ingredients": [
    { "name": "Spaghetti", "amount": 200, "unit": "g" },
    { "name": "Garlic", "amount": 2, "unit": "clove", "note": "sliced" },
    { "name": "Egg", "amount": 4 },
    { "name": "Black pepper" }
  ],
  "steps": [
    "Boil the spaghetti in salted water."
  ]
}
```

Ingredient fields: `name` (as written), `amount`, `unit` (always written except for a plain
count), `note`, `optional: true`, `aisle` (only for a name the dictionary doesn't know).
Unknown fields are rejected. Steps keep the user's words ("stock") even when the
dictionary name differs ("Vegetable broth").

## Tags and cuisines in use

Tags: `quick` (about 30 min or less in total), `dinner`, `breakfast`, `comfort`, `no-cook`, `pasta`,
`soup`, `salad`, `make-ahead`, `classic`. Reuse these; add a new one only if it would be
useful as a filter.
Cuisines: American, British, Chinese, French, Greek, Indian, Italian, Japanese, Mexican,
Middle Eastern. Reuse the same spelling.

## What `cargo test` checks

id = file name, unique; title, author, servings ≥ 1, non-empty steps and no `TODO`;
every ingredient line has a name, an amount > 0 or none, no unit without an amount, no
empty fields; dictionary units are `g`/`ml`/`pcs`, categories are aisles, `per_piece` > 0,
no name or alias points at two entries; the image file exists; the `vegetarian` tag is
never on a recipe with meat or fish, and is required when the dictionary knows every
ingredient and none is. Unknown names and units that don't add up are not failures
(`cargo run --bin ingredients`).
