use std::collections::HashMap;

use shared::dtos::LinkDto;
use sqlx::SqlitePool;

pub struct LinkRepository<'a> {
    pool: &'a SqlitePool,
}

type Result<T> = sqlx::Result<T>;

impl<'a> LinkRepository<'a> {
    pub fn new(pool: &'a SqlitePool) -> Self {
        Self { pool }
    }

    /// Store a shared link from the PWA
    pub async fn insert_link(&self, url: &str, title: Option<&str>) -> Result<()> {
        sqlx::query!("INSERT OR IGNORE INTO link (url, name) VALUES (?, ?)", url, title)
            .execute(self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_links(&self) -> Result<Vec<LinkDto>> {
        let selected = sqlx::query!(
            r#"
                SELECT url, l.name as title, l.created_at, lt.name as tag_name FROM link l 
                LEFT JOIN link_tag lt on lt.link_url = l.url
            "#
        )
        .fetch_all(self.pool)
        .await?;

        Ok(selected
            .into_iter()
            .fold(HashMap::new(), |mut map, row| {
                let entry = map.entry(row.url.clone()).or_insert_with(|| LinkDto {
                    url: row.url,
                    created_at: row.created_at,
                    title: row.title,
                    tags: Vec::new(),
                });

                if let Some(tag) = row.tag_name {
                    entry.tags.push(tag);
                }
                map
            })
            .into_values()
            .collect())
    }

    pub async fn delete_link(&self, url: &str) -> Result<()> {
        sqlx::query!("DELETE FROM link WHERE url = ?", url)
            .execute(self.pool)
            .await
            .map(|_| ())
    }
}
