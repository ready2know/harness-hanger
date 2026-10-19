//! Plugin marketplaces: the catalog of ones a person added, and a preview of what each contains.

mod catalog;
pub mod discover;
mod manifest;
mod scan;
mod source;

pub(crate) use catalog::encode_segment;
pub use catalog::{
    Catalog, CatalogPlugin, CatalogPlugins, ComponentDetails, ComponentFile, MarketplaceDetails,
    MarketplaceSummary, PluginDetails,
};
pub(crate) use scan::standalone;
pub use scan::{
    Component, ComponentKind, MarketplacePreview, PluginPreview, PluginSource, ScanWarning, preview,
};
pub use source::MarketplaceSource;
pub(crate) use source::github as parse_github_repo;
