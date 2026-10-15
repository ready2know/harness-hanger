//! Plugin marketplaces: the catalog of ones a person added, and a preview of what each contains.

mod catalog;
mod manifest;
mod scan;
mod source;

pub use catalog::{Catalog, MarketplaceDetails, MarketplaceSummary};
pub use scan::{Component, MarketplacePreview, PluginPreview, PluginSource, ScanWarning, preview};
pub use source::MarketplaceSource;
