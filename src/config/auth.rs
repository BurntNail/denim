use std::collections::HashMap;
use std::ops::Range;
use std::sync::{Arc, LazyLock};
use rand::{rng, Rng};
use s3::Bucket;
use s3::error::S3Error;
use serde::{Deserialize, Serialize};
use snafu::{OptionExt, ResultExt};
use crate::config::important_item::{ImportantItem, ImportantItemTy};
use crate::error::{DenimError, DenimResult, GeneratePasswordSnafu, RmpSerdeDecodeSnafu, RmpSerdeEncodeSnafu, S3Snafu};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    pub word_len_range: Range<usize>,
    pub numbers_range: Range<usize>,
}

impl AuthConfig {
    fn words() -> &'static HashMap<usize, Vec<Arc<str>>> {
        static WORDS: LazyLock<HashMap<usize, Vec<Arc<str>>>> = LazyLock::new(|| {
            let all_words = include_str!("words.txt");
            let mut map: HashMap<usize, Vec<Arc<str>>> = HashMap::new();

            for (len, word) in all_words
                .lines()
                .map(str::trim)
                .map(|word| (word.len(), word))
            {
                map.entry(len).or_default().push(word.into());
            }

            map
        });
        &WORDS
    }
    
    pub fn generate(&self) -> DenimResult<String> {
        let mut rng = rng();

        let word_len = rng.random_range(self.word_len_range.clone());
        let list_to_pick_from = Self::words().get(&word_len).context(GeneratePasswordSnafu)?;
        let chosen_word = list_to_pick_from[rng.random_range(0..list_to_pick_from.len())].clone();

        let chosen_number = rng.random_range(self.numbers_range.clone());

        Ok(format!("{chosen_word}_{chosen_number}"))
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        let _ = Self::words();
        let default_word_len_range = 5..9;
        let default_numbers_range = 1_000..10_000;

        Self {
            word_len_range: default_word_len_range,
            numbers_range: default_numbers_range,
        }
    }
}

impl ImportantItem for AuthConfig {
    const TY: ImportantItemTy = ImportantItemTy::AuthConfig;

    async fn get_from_bucket(bucket: &Bucket) -> DenimResult<Option<Self>> {
        let rsp = match bucket.get_object("auth_config.bin").await {
            Err(S3Error::HttpFailWithBody(404, _)) => return Ok(None),
            Err(e) => return Err(DenimError::S3 {source: e}),
            Ok(rsp) => rsp,
        };
        
        rmp_serde::from_slice(rsp.bytes()).context(RmpSerdeDecodeSnafu).map(Some)
    }

    async fn save_to_bucket(&self, bucket: &Bucket) -> DenimResult<()> {
        let serialised = rmp_serde::to_vec(self).context(RmpSerdeEncodeSnafu)?;
        bucket.put_object_with_content_type(
            "auth_config.bin",
            &serialised,
            "application/octet-stream"
        ).await.context(S3Snafu)?;
        Ok(())
    }
}