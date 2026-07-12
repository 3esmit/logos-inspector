mod store;

pub mod engine;
pub mod model;
pub mod report;
pub mod service;
pub mod source;
pub mod worker;

use std::path::Path;

pub use engine::*;
pub use model::*;
pub use report::*;
pub use service::*;
pub use source::*;
pub use worker::*;

use store::ZoneCatalogStore;

pub struct ZoneCatalog {
    store: ZoneCatalogStore,
}

impl ZoneCatalog {
    pub fn create(path: impl AsRef<Path>, metadata: CatalogMetadata) -> CatalogResult<Self> {
        Ok(Self {
            store: ZoneCatalogStore::create(path.as_ref(), metadata)?,
        })
    }

    pub fn open(path: impl AsRef<Path>) -> CatalogResult<Self> {
        Ok(Self {
            store: ZoneCatalogStore::open(path.as_ref())?,
        })
    }

    pub fn open_read_only(path: impl AsRef<Path>) -> CatalogResult<Self> {
        Ok(Self {
            store: ZoneCatalogStore::open_read_only(path.as_ref())?,
        })
    }

    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.store.is_read_only()
    }

    pub fn snapshot(&self) -> CatalogResult<CatalogSnapshot> {
        self.store.snapshot()
    }

    pub fn commit_batch(&self, batch: CatalogBatch) -> CatalogResult<CatalogSnapshot> {
        self.store.commit_batch(batch)
    }
}
