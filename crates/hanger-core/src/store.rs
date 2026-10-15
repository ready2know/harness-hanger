//! harness-hanger's own SQLite database, for data that the harness does not keep.

use std::fs;
use std::path::Path;

use rusqlite::{Connection, TransactionBehavior};

use crate::{Error, Result};

const FILE_NAME: &str = "harness-hanger.db";

/// Schema changes in order. `PRAGMA user_version` counts how many have been applied, so only
/// append to this list.
const MIGRATIONS: &[&str] = &[
    "CREATE TABLE marketplace (
        -- AUTOINCREMENT: a removed marketplace's ID is never given to another one.
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        kind TEXT NOT NULL,
        location TEXT NOT NULL,
        added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
        UNIQUE (kind, location)
    )",
    "CREATE TABLE project (
        -- AUTOINCREMENT: a removed project's ID is never given to another one.
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        path TEXT NOT NULL UNIQUE,
        added_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
    )",
];

/// Opens the database in `data_dir`, creating it and applying pending migrations as needed.
pub(crate) fn open(data_dir: &Path) -> Result<Connection> {
    fs::create_dir_all(data_dir).map_err(|source| Error::io(data_dir, source))?;
    let mut connection = Connection::open(data_dir.join(FILE_NAME))?;
    migrate(&mut connection)?;
    Ok(connection)
}

fn migrate(connection: &mut Connection) -> Result<()> {
    // Immediate, so that the CLI and the app cannot both apply the same migration.
    let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let applied: i64 = transaction.pragma_query_value(None, "user_version", |row| row.get(0))?;
    for (version, sql) in (1..)
        .zip(MIGRATIONS)
        .skip(usize::try_from(applied).unwrap_or(0))
    {
        transaction.execute_batch(sql)?;
        transaction.pragma_update(None, "user_version", version)?;
    }
    transaction.commit()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_the_schema_once() {
        let dir = tempfile::tempdir().unwrap();
        let data_dir = dir.path().join("nested");

        open(&data_dir).unwrap();
        let connection = open(&data_dir).unwrap();

        let version: i64 = connection
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, i64::try_from(MIGRATIONS.len()).unwrap());
        connection
            .execute(
                "INSERT INTO marketplace (kind, location) VALUES ('local', '/x')",
                [],
            )
            .unwrap();
    }
}
