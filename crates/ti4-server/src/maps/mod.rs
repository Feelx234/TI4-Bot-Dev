//! Map generation and template management.

pub mod build;
pub mod catalog;
pub mod template_loader;

pub use build::{TemplateError, build_template_galaxy, default_template_for, placeholder_homes};
pub use catalog::{MapChoice, MapChoiceView, MapPreview, SeatPreview};
pub use template_loader::{MapTemplate, MapTemplateSummary, TemplateLoader};
