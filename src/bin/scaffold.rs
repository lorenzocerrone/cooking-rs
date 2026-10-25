//! Host-only recipe scaffolder: a plain-text draft in, `recipes/<id>.json` and
//! `images/<id>.svg` out. The parsing and rendering live in
//! `cooking_rs::scaffold`; this file is only arguments and file I/O.
//!
//! ```sh
//! cargo run --bin scaffold -- draft.txt [--dry-run] [--force]
//! cargo run --bin scaffold -- - < draft.txt
//! ```

use std::io::Read;
use std::path::Path;
use std::process::ExitCode;

use cooking_rs::recipes::load_catalog;
use cooking_rs::scaffold::{parse_draft, render_json, render_svg};

const USAGE: &str = "usage: scaffold <draft.txt | -> [--dry-run] [--force]";

fn main() -> ExitCode {
    let mut input = None;
    let (mut dry_run, mut force) = (false, false);
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--dry-run" => dry_run = true,
            "--force" => force = true,
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            _ if input.is_none() => input = Some(arg),
            _ => return fail(USAGE),
        }
    }
    let Some(input) = input else {
        return fail(USAGE);
    };
    let text = match read_input(&input) {
        Ok(text) => text,
        Err(e) => return fail(&format!("{input}: {e}")),
    };

    let draft = match parse_draft(&text, &load_catalog()) {
        Ok(draft) => draft,
        Err(problems) => {
            for p in &problems {
                eprintln!("{input}:{}: {}", p.line, p.message);
            }
            return fail(&format!("{} problem(s), nothing written", problems.len()));
        }
    };
    for w in &draft.warnings {
        eprintln!("warning: {w}");
    }

    let recipe = &draft.recipe;
    let json = render_json(recipe);
    let svg = render_svg(recipe);
    if dry_run {
        print!("{json}\n{svg}");
        return ExitCode::SUCCESS;
    }

    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let json_path = root.join("recipes").join(format!("{}.json", recipe.id));
    let svg_path = root.join(format!("images/{}.svg", recipe.id));
    for path in [&json_path, &svg_path] {
        if path.exists() && !force {
            return fail(&format!(
                "{} exists (use --force to overwrite)",
                path.display()
            ));
        }
    }
    for (path, contents) in [(&json_path, &json), (&svg_path, &svg)] {
        if let Err(e) = std::fs::write(path, contents) {
            return fail(&format!("{}: {e}", path.display()));
        }
        println!("wrote {}", path.display());
    }
    println!("next: `cargo test` validates it");
    ExitCode::SUCCESS
}

fn read_input(input: &str) -> std::io::Result<String> {
    if input == "-" {
        let mut text = String::new();
        std::io::stdin().read_to_string(&mut text)?;
        Ok(text)
    } else {
        std::fs::read_to_string(input)
    }
}

fn fail(message: &str) -> ExitCode {
    eprintln!("error: {message}");
    ExitCode::FAILURE
}
