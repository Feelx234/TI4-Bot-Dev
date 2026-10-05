//! Map generation and template management.

pub mod build;
pub mod template_loader;

pub use build::{TemplateError, build_template_galaxy, default_template_for};
pub use template_loader::{MapTemplate, MapTemplateSummary, TemplateLoader};
