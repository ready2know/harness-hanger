//! Plugin marketplaces: the catalog of ones a person added, and a preview of what each contains.

mod catalog;
mod manifest;
mod scan;
mod source;

pub use catalog::{
    Catalog, CatalogPlugin, CatalogPlugins, ComponentDetails, ComponentFile, MarketplaceDetails,
    MarketplaceSummary, PluginDetails,
};
pub(crate) use scan::standalone;
pub use scan::{
    Component, ComponentKind, MarketplacePreview, PluginPreview, PluginSource, ScanWarning, preview,
};
pub use source::MarketplaceSource;
