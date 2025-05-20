use crate::error::DenimResult;
use s3::Bucket;
use std::{
    fmt::Debug,
    sync::{Arc, OnceLock},
};

#[derive(Copy, Clone, Debug)]
pub enum ImportantItemTy {
    Bucket,
    DateLocaleConfig,
    AuthConfig,
}

#[derive(Debug)]
pub struct ImportantItemContainer<T: ImportantItem>(Arc<OnceLock<Arc<T>>>);

impl<T: ImportantItem> ImportantItemContainer<T> {
    pub fn new() -> Self {
        Self(Arc::new(OnceLock::new()))
    }

    #[allow(dead_code)]
    pub async fn try_new_from_bucket(bucket: &Bucket) -> DenimResult<Self> {
        match T::get_from_bucket(bucket).await {
            Ok(None) => Ok(Self::new()),
            Err(e) => Err(e),
            Ok(Some(worked)) => {
                let ol = OnceLock::new();
                let _ = ol.set(Arc::new(worked)); //not quite sure how this one could fail lol
                Ok(Self(Arc::new(ol)))
            }
        }
    }

    ///returns whether something is there now
    #[allow(clippy::future_not_send)]
    pub async fn try_set_from_bucket(&self, bucket: &Bucket) -> DenimResult<bool> {
        if self.exists() {
            return Ok(true);
        }

        match T::get_from_bucket(bucket).await {
            Ok(None) => Ok(false),
            Err(e) => Err(e),
            Ok(Some(found)) => {
                info!(ty = ?<T as ImportantItem>::TY, "Loaded important item");
                let _ = self.0.set(Arc::new(found));
                Ok(true)
            }
        }
    }

    pub fn exists(&self) -> bool {
        self.0.get().is_some()
    }

    pub fn get(&self) -> DenimResult<Arc<T>> {
        self.0
            .get()
            .cloned()
            .ok_or_else(|| <T as ImportantItem>::TY.into())
    }

    pub fn set(&self, item: T) -> Result<(), Arc<T>> {
        self.0.set(Arc::new(item))
    }

    #[allow(clippy::future_not_send)]
    pub async fn save(&self, bucket: &Bucket) -> DenimResult<()> {
        if let Some(item) = self.0.get() {
            item.save_to_bucket(bucket).await
        } else {
            Ok(())
        }
    }
}

impl<T: ImportantItem> Clone for ImportantItemContainer<T> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

pub trait ImportantItem: Sized {
    const TY: ImportantItemTy;

    ///result: did we fail to get it from the bucket?
    ///inner option: was this item in the bucket?
    ///
    ///if this item can't be retreived from the bucket (like if it is the bucket), just return `Ok(None)`
    async fn get_from_bucket(bucket: &Bucket) -> DenimResult<Option<Self>>;
    async fn save_to_bucket(&self, bucket: &Bucket) -> DenimResult<()>;
}

impl ImportantItem for Bucket {
    const TY: ImportantItemTy = ImportantItemTy::Bucket;

    async fn get_from_bucket(_bucket: &Bucket) -> DenimResult<Option<Self>> {
        Ok(None)
    }

    async fn save_to_bucket(&self, _bucket: &Bucket) -> DenimResult<()> {
        Ok(())
    }
}
