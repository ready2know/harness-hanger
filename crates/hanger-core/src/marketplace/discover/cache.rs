//! The local index of the marketplaces that Discover found, and the searches that found them.

use std::collections::HashMap;

use rusqlite::{Connection, OptionalExtension, params};

use super::{Record, SearchMethod};
use crate::Result;

/// Searches, and marketplaces that no search found again, are forgotten after this long, so that
/// one that was deleted or made private stops showing up.
const KEEP: i64 = 30 * 24 * 60 * 60;

/// A search that GitHub answered.
pub(super) struct Search {
    pub method: SearchMethod,
    /// The keys of the repositories GitHub found, in its order.
    pub repos: Vec<String>,
    pub total_count: Option<u64>,
    /// Unix seconds.
    pub searched_at: i64,
}

/// A marketplace in the index.
pub(super) struct Indexed {
    pub record: Record,
    /// Unix seconds.
    pub fetched_at: i64,
}

/// The last search for `query` by `method`.
pub(super) fn search(
    connection: &Connection,
    method: SearchMethod,
    query: &str,
) -> Result<Option<Search>> {
    let row = connection
        .query_row(
            "SELECT repos, total_count, searched_at FROM discover_search
             WHERE method = ?1 AND query = ?2",
            params![method.as_str(), query],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<i64>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    Ok(row.and_then(|(repos, total_count, searched_at)| {
        Some(Search {
            method,
            repos: serde_json::from_str(&repos).ok()?,
            total_count: total_count.and_then(|count| u64::try_from(count).ok()),
            searched_at,
        })
    }))
}

/// Saves the marketplaces fetched at `fetched_at` and, when given, the search that found them as
/// `(query, search)`, in one transaction, and forgets what is too old.
pub(super) fn save(
    connection: &Connection,
    records: &[Record],
    fetched_at: i64,
    search: Option<(&str, &Search)>,
) -> Result<()> {
    let transaction = connection.unchecked_transaction()?;
    for record in records {
        let Ok(data) = serde_json::to_string(record) else {
            continue;
        };
        transaction.execute(
            "INSERT OR REPLACE INTO discover_repo (key, data, fetched_at) VALUES (?1, ?2, ?3)",
            params![record.key(), data, fetched_at],
        )?;
    }
    if let Some((query, search)) = search {
        transaction.execute(
            "INSERT OR REPLACE INTO discover_search (method, query, repos, total_count, searched_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                search.method.as_str(),
                query,
                serde_json::to_string(&search.repos).unwrap_or_else(|_| "[]".to_owned()),
                search
                    .total_count
                    .and_then(|count| i64::try_from(count).ok()),
                search.searched_at,
            ],
        )?;
    }
    transaction.execute(
        "DELETE FROM discover_search WHERE searched_at < ?1",
        [fetched_at - KEEP],
    )?;
    transaction.execute(
        "DELETE FROM discover_repo WHERE fetched_at < ?1",
        [fetched_at - KEEP],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Every marketplace in the index, by key. A row that cannot be read, such as one that a newer
/// version wrote, is left out.
pub(super) fn repos(connection: &Connection) -> Result<HashMap<String, Indexed>> {
    let mut statement = connection.prepare("SELECT key, data, fetched_at FROM discover_repo")?;
    let rows = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(rows
        .into_iter()
        .filter_map(|(key, data, fetched_at)| {
            let record = serde_json::from_str(&data).ok()?;
            Some((key, Indexed { record, fetched_at }))
        })
        .collect())
}

/// The ID of each GitHub marketplace in the catalog, by `owner/repo` in lowercase.
pub(super) fn catalog_ids(connection: &Connection) -> Result<HashMap<String, i64>> {
    let mut statement =
        connection.prepare("SELECT id, location FROM marketplace WHERE kind = 'github'")?;
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(1)?.to_ascii_lowercase(), row.get(0)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}
