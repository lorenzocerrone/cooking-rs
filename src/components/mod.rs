use crate::date::Date;
use crate::models::MealPlan;

mod add_to_week;
mod day_pick;
mod icons;
mod modal;
mod nav;
mod recipe_filters;
mod recipe_picker;
mod stepper;
mod toast;

pub use add_to_week::AddToWeek;
pub use day_pick::DayPick;
pub use icons::{IconBack, IconBook, IconCalendar, IconCart, IconClose, IconPlus};
pub use modal::Modal;
pub use nav::Nav;
pub use recipe_filters::RecipeFilters;
pub use recipe_picker::RecipePicker;
pub use stepper::Stepper;
pub use toast::Toast;

/// Days the plan always shows from today, and the "Add to week" sheet offers.
pub const ROLLING_DAYS: i32 = 14;

/// Today's date from the browser's local clock.
pub fn today() -> Date {
    let now = js_sys::Date::new_0();
    Date::from_ymd(
        now.get_full_year() as i32,
        now.get_month() + 1,
        now.get_date(),
    )
    .expect("the browser clock gives a valid date")
}

/// The days to list from `today`: [`ROLLING_DAYS`] of them, or through the last
/// planned day if that is later.
pub fn rolling_dates(today: Date, plan: &MealPlan) -> Vec<Date> {
    let last = plan
        .last_date()
        .map_or(0, |d| today.days_until(d))
        .max(ROLLING_DAYS - 1);
    (0..=last).map(|n| today.add_days(n)).collect()
}
