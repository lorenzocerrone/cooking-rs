// Checks the redirect's mapping of old addresses to new ones: `node redirect/test.mjs`.
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const html = readFileSync(new URL("./index.html", import.meta.url), "utf8");
const script = /<script>([\s\S]*?)<\/script>/.exec(html)[1];
// Run the page's script with a stand-in for `location` that records where it goes.
function redirectFrom(pathname, search = "", hash = "") {
  let went = null;
  const location = { pathname, search, hash, replace: (to) => (went = to) };
  new Function("location", script)(location);
  return went;
}

const P = "eyJkYXRlcyI6eyIyMDI2LTExLTE2IjpbImNhcmJvbmFyYSJdfX0";
const cases = [
  // The home-screen icon and the bare site.
  ["/cooking-rs/", "", "", "/menu-lock/#/"],
  ["/cooking-rs", "", "", "/menu-lock/#/"],
  ["/cooking-rs/index.html", "", "", "/menu-lock/#/"],
  // Each page keeps its name and carries the plan.
  ["/cooking-rs/", "", `#plan=${P}`, `/menu-lock/#/?plan=${P}`],
  ["/cooking-rs/planner", "", `#plan=${P}`, `/menu-lock/#/planner?plan=${P}`],
  ["/cooking-rs/grocery", "", `#plan=${P}`, `/menu-lock/#/grocery?plan=${P}`],
  ["/cooking-rs/grocery/", "", "", "/menu-lock/#/grocery"],
  ["/cooking-rs/recipe/lentil_dal", "", `#plan=${P}`, `/menu-lock/#/recipe/lentil_dal?plan=${P}`],
  // The recipe list's search and filters keep their names.
  ["/cooking-rs/", "?diet=veg&tag=quick,dinner&time=20", `#plan=${P}`, `/menu-lock/#/?diet=veg&tag=quick,dinner&time=20&plan=${P}`],
  ["/cooking-rs/", "?q=dal%20%26%20rice&by=Anna%20Maria", "", "/menu-lock/#/?q=dal%20%26%20rice&by=Anna%20Maria"],
  // Anything else in the query is dropped, and other pages take no filters.
  ["/cooking-rs/", "?path=%2Fplanner&x=1&q=", "", "/menu-lock/#/"],
  ["/cooking-rs/planner", "?tag=quick", "", "/menu-lock/#/planner"],
  // The recipe editor was not ported; unknown pages go to the recipe list.
  ["/cooking-rs/new", "", `#plan=${P}`, `/menu-lock/#/?plan=${P}`],
  ["/cooking-rs/nope/deeper", "", "", "/menu-lock/#/"],
  // A fragment that is not a plan is ignored.
  ["/cooking-rs/planner", "", "#plan=<script>", "/menu-lock/#/planner"],
  ["/cooking-rs/planner", "", "#top", "/menu-lock/#/planner"],
  // Pull-request previews of this repo are left alone.
  ["/cooking-rs/pr-preview/pr-12/planner", "", `#plan=${P}`, null],
  ["/cooking-rs/pr-preview/pr-12/", "", "", null],
];

for (const [pathname, search, hash, expected] of cases) {
  assert.equal(redirectFrom(pathname, search, hash), expected, `${pathname}${search}${hash}`);
}
console.log(`redirect: ${cases.length} addresses map correctly`);
