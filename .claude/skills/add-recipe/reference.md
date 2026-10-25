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
3 cloves garlic                 ← piece label of the catalog entry
1/2 can coconut milk            ← fractions and decimal commas work
chives (optional)               ← "(optional)" at the end
salt                            ← no amount = to taste

Steps                           ← or "Method"
1. Soften the onion and garlic. ← "1." / "1)" / "-" numbering is stripped
```

Headers allowed: `id` (override the slug), `author`, `servings`/`serves`, `cuisine`,
`description`, `tags`, `prep`/`prep_time`, `cook`/`cook_time`. Anything else is an error.
No steps → a `TODO` step that `cargo test` rejects, so always write steps.
The scaffolder adds/removes the `vegetarian` tag itself; don't put it in `tags`.

## Amounts and units

- A leading number is the amount; the next word is the unit if it is a unit spelling
  (`g kg ml l tsp tbsp cup pcs`, also `grams`, `tablespoons`, `pieces`…) or the
  catalog entry's piece label (`clove(s)`, `can(s)`, `sprig(s)`, `thumb(s)`, `fillet(s)`).
  Otherwise it's part of the name: `1 red onion` is one `red_onion`.
- A bare number counts pieces: `2 onion` = two onions. That needs either a `pcs`
  shopping unit or a `count.per_piece` on the entry, else it's an error.
- **Units must convert into the entry's shopping unit.** Mass (g, kg) ↔ mass, volume
  (ml, l, tsp, tbsp, cup) ↔ volume, pieces ↔ anything via `per_piece`. There are no
  densities: `1 tbsp butter` fails because butter is shopped in g. Convert yourself and
  show it in the preview mapping. Rough values:
  - ground spice 1 tsp ≈ 2 g, 1 tbsp ≈ 6 g; dried herbs 1 tsp ≈ 1 g
  - salt 1 tsp ≈ 6 g; sugar 1 tbsp ≈ 12 g, 1 cup ≈ 200 g; flour 1 cup ≈ 125 g
  - butter 1 tbsp ≈ 14 g, a knob ≈ 15 g; honey 1 tbsp ≈ 21 g; peanut butter 1 tbsp ≈ 16 g
  - rice 1 cup ≈ 190 g; oats 1 cup ≈ 90 g; grated cheese 1 cup ≈ 100 g
  - lentils / dried beans 1 cup ≈ 200 g; frozen peas 1 cup ≈ 140 g
  - US/imperial: 1 oz = 28 g, 1 lb = 450 g, 1 fl oz = 30 ml, 1 pint = 470 ml (US)
- The scaffolder strips nothing before the name: "a knob of butter", "juice of half a
  lemon" become unknown ingredients. Rewrite every line yourself into
  `amount unit name, note`: "juice of half a lemon" → `0.5 lemon, juiced`.
- Vague amounts with a usual size get one, listed under Guesses: a knob ≈ 15 g, a handful
  of leaves ≈ 30 g. Those without ("a pinch", "a splash", "to taste", "for frying") →
  no amount.
- Keep the recipe's own servings; don't rescale on add.

## Mapping to the catalog

- `find_ingredient` matches a key or display name, case-insensitive, singular/plural —
  and nothing else. So write the catalog's name in the draft (`scallion`, not "spring
  onion"; `chopped tomatoes`, `lentils` for red lentils, `olive oil`).
- Prefer an existing key whenever the shopping is the same: "spring onions"/"green
  onions" → `scallion`; "tinned tomatoes" → `chopped tomatoes` (unit `can`); "tin" →
  `can`; "extra-virgin olive oil" → `olive oil`; "minced beef" → `ground beef`;
  "courgette" → `zucchini`. Put a meaningful variety in the note instead
  (`300 g lentils, red`).
- The scaffolder's "did you mean …?" hints list close catalog matches; check them first.
  They miss synonyms ("spring onions" gets no hint for `scallion`), so check the
  catalog yourself too.
- Beware the "or, if `tin` is a piece label" stub: for `2 tins chickpeas` it proposes a
  new `chickpeas` entry with label `tin`, but `chickpeas` already exists with label
  `can`. Fix the draft (`2 cans chickpeas`), never add the stub.
- Create a new entry only when it's a different thing to buy (curry leaves, feta vs
  ricotta). Show every new entry in the preview.

## Catalog entries (`ingredients.json`)

```json
"coconut_milk":     { "name": "Coconut milk",     "unit": "ml",  "category": "Pantry", "count": { "label": "can", "per_piece": 400 } },
```

- key: lowercase `a-z 0-9 _`, singular unless the thing is plural (`chickpeas`, `peas`).
- `name`: shown on the grocery list, capitalised ("Curry leaves").
- `unit`: the shopping unit, one of `g`, `ml`, `pcs`. Weighed things in `g`, liquids in
  `ml`, things bought by the piece in `pcs` (lemon, egg, tortilla). Spices, honey and
  butter in `g`.
- `category`: exactly one aisle, in this order:
  `Produce, Meat, Fish, Dairy & Eggs, Pasta & Grains, Bakery, Legumes, Pantry, Frozen,
  Spices & Oils`. Meat and Fish make a recipe non-vegetarian.
- `count` (optional): `per_piece` = grams/ml of one piece (onion 150, garlic clove 4,
  can 400) so bare numbers and labels convert; `label` = what one piece is called
  ("clove", "can", "sprig"); `plural` only if not label + "s". A `pcs` entry with a
  `count` needs a `label` or a `per_piece`.
- The stub uses the draft's spelling: singularise key and name (`carrots` → `carrot`,
  "Carrot"); the draft can stay plural, matching handles it.
- The stub's `"category": "?"` always needs a real aisle; its `per_piece: 100` (present
  only when the line counted pieces) always needs a real piece size.
- Append the line at the end of its aisle's block (blocks are separated by blank lines,
  in aisle order), padding so the columns line up with the neighbours.

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
    { "ingredient": "spaghetti", "amount": 200 },
    { "ingredient": "garlic", "amount": 2, "unit": "clove", "note": "sliced" },
    { "ingredient": "black_pepper" }
  ],
  "steps": [
    "Boil the spaghetti in salted water."
  ]
}
```

Ingredient fields: `ingredient` (key), `amount`, `unit` (only when not the shopping unit),
`note`, `optional: true`. Unknown fields are rejected. Steps keep the user's words
("stock") even when the catalog name differs ("Vegetable broth").

## Tags and cuisines in use

Tags: `quick` (about 30 min or less in total), `dinner`, `breakfast`, `comfort`, `no-cook`, `pasta`,
`soup`, `salad`, `make-ahead`, `classic`. Reuse these; add a new one only if it would be
useful as a filter.
Cuisines: American, British, Chinese, French, Greek, Indian, Italian, Japanese, Mexican,
Middle Eastern. Reuse the same spelling.

## What `cargo test` checks

id = file name, unique; title, author, servings ≥ 1, non-empty steps and no `TODO`;
every ingredient exists in the catalog; every amount's unit converts; catalog units are
`g`/`ml`/`pcs`, categories are aisles, `per_piece` > 0; the image file exists; the
`vegetarian` tag is set exactly when no ingredient is from Meat or Fish.
