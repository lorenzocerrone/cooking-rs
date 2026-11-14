"""Write dist/404.html, the page GitHub Pages serves for any unknown path.

It is the app's index.html (so a hard refresh of /cooking-rs/planner loads the
app) plus a tiny script for pull-request previews: Pages serves only this one
site-wide 404.html, so a deep link inside a preview (/cooking-rs/pr-preview/
pr-12/planner) is sent to that preview's root with ?path=/planner, which the
app puts back in the address bar (see redirected_path in src/app.rs).

Usage: python3 .github/scripts/spa_fallback.py dist
"""

import pathlib
import sys

REDIRECT = """<script>
      (function () {
        var m = location.pathname.match(/^(\\/[^/]+\\/pr-preview\\/pr-\\d+)(\\/.+)$/);
        if (m) {
          location.replace(m[1] + "/?path=" + encodeURIComponent(m[2] + location.search) + location.hash);
        }
      })();
    </script>"""


def main(dist: str) -> None:
    root = pathlib.Path(dist)
    index = (root / "index.html").read_text(encoding="utf-8")
    if "<head>" not in index:
        sys.exit("index.html has no <head>")
    page = index.replace("<head>", "<head>\n    " + REDIRECT, 1)
    (root / "404.html").write_text(page, encoding="utf-8")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "dist")
