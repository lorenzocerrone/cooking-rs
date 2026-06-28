use std::collections::BTreeMap;

use leptos::prelude::*;

use crate::models::{CatalogIngredient, IngredientCatalog};
use crate::recipes::load_data;
use crate::shopping::category_sort_key;

/// A read-only view of the full ingredient catalog, grouped by category.
#[component]
pub fn IngredientsPage() -> impl IntoView {
    let data = Resource::new(|| (), |_| async { load_data().await });

    view! {
        <h1>"Ingredients"</h1>
        <p class="muted">"The catalog every recipe draws from."</p>
        <Suspense fallback=|| view! { <p class="muted">"Loading ingredients…"</p> }>
            {move || data.get().map(|d| view! { <Catalog catalog=d.catalog /> })}
        </Suspense>
    }
}

#[component]
fn Catalog(catalog: IngredientCatalog) -> impl IntoView {
    if catalog.0.is_empty() {
        return view! { <p class="muted">"No ingredients found."</p> }.into_any();
    }

    // Group entries by category (sorted by name within each).
    let mut by_category: BTreeMap<String, Vec<CatalogIngredient>> = BTreeMap::new();
    for entry in catalog.0.into_values() {
        by_category
            .entry(entry.category.clone())
            .or_default()
            .push(entry);
    }
    let mut sections: Vec<(String, Vec<CatalogIngredient>)> = by_category.into_iter().collect();
    for (_, items) in sections.iter_mut() {
        items.sort_by(|a, b| a.name.cmp(&b.name));
    }
    sections.sort_by_key(|(name, _)| category_sort_key(name));

    view! {
        <div class="ingredient-sections">
            {sections
                .into_iter()
                .map(|(name, items)| {
                    view! {
                        <section>
                            <h2>{name}</h2>
                            <table class="ingredient-table">
                                <thead>
                                    <tr>
                                        <th>"Ingredient"</th>
                                        <th>"Unit"</th>
                                        <th>"Calories"</th>
                                        <th>"Protein"</th>
                                        <th>"Carbs"</th>
                                        <th>"Fat"</th>
                                    </tr>
                                </thead>
                                <tbody>
                                    {items
                                        .into_iter()
                                        .map(|ing| {
                                            let m = ing.macros;
                                            view! {
                                                <tr>
                                                    <td>{ing.name}</td>
                                                    <td>{ing.unit}</td>
                                                    <td>{cell(m.as_ref().map(|m| m.calories))}</td>
                                                    <td>{cell(m.as_ref().map(|m| m.protein))}</td>
                                                    <td>{cell(m.as_ref().map(|m| m.carbs))}</td>
                                                    <td>{cell(m.as_ref().map(|m| m.fat))}</td>
                                                </tr>
                                            }
                                        })
                                        .collect_view()}
                                </tbody>
                            </table>
                        </section>
                    }
                })
                .collect_view()}
        </div>
    }
    .into_any()
}

/// Render a macro cell, or "—" when no macro data is available.
fn cell(value: Option<f64>) -> String {
    match value {
        Some(v) => format!("{}", (v * 10.0).round() / 10.0),
        None => "—".to_string(),
    }
}
