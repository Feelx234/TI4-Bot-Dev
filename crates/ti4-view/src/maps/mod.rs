//! Map templates: the predefined board layouts and the galaxy built from one.

pub mod build;
pub mod catalog;
pub mod template_loader;

pub use build::{TemplateError, build_template_galaxy, default_template_for, placeholder_homes};
pub use catalog::{MapChoice, MapChoiceView, MapPreview, SeatPreview};
pub use template_loader::{MapTemplate, MapTemplateSummary, TemplateLoader};
