//! Map generation and template management. The templates themselves are in `ti4-view`.

pub mod catalog;

pub use ti4_view::maps::{build, template_loader};

#[cfg(test)]
mod community_tests;

pub use build::{TemplateError, build_template_galaxy, default_template_for, placeholder_homes};
pub use catalog::{MapChoice, MapChoiceView, MapPreview, SeatPreview};
pub use template_loader::{MapTemplate, MapTemplateSummary, TemplateLoader};
