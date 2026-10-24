mod add_to_week;
mod icons;
mod modal;
mod nav;
mod recipe_filters;
mod recipe_picker;
mod stepper;
mod toast;

pub use add_to_week::AddToWeek;
pub use icons::{IconBack, IconBook, IconCalendar, IconCart, IconClose, IconPlus};
pub use modal::Modal;
pub use nav::Nav;
pub use recipe_filters::RecipeFilters;
pub use recipe_picker::RecipePicker;
pub use stepper::Stepper;
pub use toast::Toast;

/// Today's index into [`crate::models::DAY_NAMES`] (Monday = 0), from the
/// browser clock.
pub fn today_index() -> usize {
    // JS: Sunday = 0 .. Saturday = 6.
    let js_day = js_sys::Date::new_0().get_day() as usize;
    (js_day + 6) % 7
}
