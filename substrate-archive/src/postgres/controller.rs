use super::batch::BatchLoader;
use super::partial::{PartialBatchLoader, PartialOptions};
use super::{BatchResponse, DatabaseType};
use crate::error::Error;
use crate::metrics::ObserverExt;
use crate::BatchOptions;
use sqlx::{Pool, Postgres};

pub struct BatchController {
    pool: Pool<Postgres>,
    database_type: DatabaseType,
}

impl BatchController {
    pub fn new(pool: Pool<Postgres>, database_type: DatabaseType) -> BatchController {
        BatchController {
            pool,
            database_type,
        }
    }

    pub async fn load(&self, options: &BatchOptions) -> Result<BatchResponse, Error> {
        let to_block = match options.to_block {
            Some(to_block) => to_block,
            None => {
                let head = self.archive_head().await?;
                match head {
                    Some(head) => head.try_into().unwrap(),
                    None => {
                        // archive is empty
                        match options.limit {
                            Some(..) => {
                                return Ok(BatchResponse {
                                    data: vec![],
                                    next_block: None,
                                })
                            }
                            None => {
                                return Ok(BatchResponse {
                                    data: vec![],
                                    next_block: Some(options.from_block),
                                })
                            }
                        }
                    }
                }
            }
        };

        match options.limit {
            Some(..) => {
                let strategy = options.loader(self.pool.clone(), self.database_type.clone());
                strategy.load().await
            }
            None => {
                let loader = BatchLoader::new(self.pool.clone(), self.database_type.clone());
                let strategy = PartialBatchLoader::new(loader);
                let options = PartialOptions {
                    from_block: options.from_block,
                    to_block,
                    include_all_blocks: options.include_all_blocks,
                    selections: options.selections.clone(),
                };
                strategy.load(&options).await
            }
        }
    }

    async fn archive_head(&self) -> Result<Option<i64>, Error> {
        let query = "SELECT height::int8 FROM block ORDER BY height DESC LIMIT 1";
        let head = sqlx::query_scalar::<_, i64>(query)
            .fetch_optional(&self.pool)
            .observe_duration("block")
            .await?;
        Ok(head)
    }
}
