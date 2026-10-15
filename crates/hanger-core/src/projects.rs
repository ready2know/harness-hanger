//! The project folders a person added, so that the app can show and change what applies in each.

use std::io;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, ErrorCode, OptionalExtension};
use serde::Serialize;

use crate::{Error, Result, paths, store};

pub struct Projects {
    connection: Connection,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: i64,
    /// The folder's name.
    pub name: String,
    pub path: PathBuf,
    /// When it was added, as an RFC 3339 UTC timestamp.
    pub added_at: String,
    /// Why the folder cannot be used, for example because it was moved.
    pub error: Option<String>,
}

impl Projects {
    /// Opens the project list kept in `data_dir`, creating it on first use.
    pub fn open(data_dir: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            connection: store::open(data_dir.as_ref())?,
        })
    }

    /// Opens the project list in the default [data directory](paths::data_dir).
    pub fn open_default() -> Result<Self> {
        Self::open(paths::data_dir()?)
    }

    pub fn list(&self) -> Result<Vec<Project>> {
        let mut statement = self
            .connection
            .prepare("SELECT id, path, added_at FROM project ORDER BY id")?;
        let projects = statement
            .query_map([], read_row)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(projects)
    }

    pub fn get(&self, id: i64) -> Result<Project> {
        self.connection
            .query_row(
                "SELECT id, path, added_at FROM project WHERE id = ?1",
                [id],
                read_row,
            )
            .optional()?
            .ok_or(Error::ProjectNotFound(id))
    }

    /// Adds the folder at `input`, a path that can start with `~`. The folder must exist.
    pub fn add(&self, input: &str) -> Result<Project> {
        let invalid = || Error::InvalidProjectPath(input.trim().to_owned());
        let path = paths::expand(input.trim()).ok_or_else(invalid)?;
        if !path.is_dir() {
            return Err(Error::io(
                path,
                io::Error::new(io::ErrorKind::NotFound, "the folder does not exist"),
            ));
        }
        let stored = path.to_str().ok_or_else(invalid)?;
        let inserted = self
            .connection
            .execute("INSERT INTO project (path) VALUES (?1)", [stored]);
        match inserted {
            Err(rusqlite::Error::SqliteFailure(error, _))
                if error.code == ErrorCode::ConstraintViolation =>
            {
                return Err(Error::AlreadyAdded(path.display().to_string()));
            }
            inserted => inserted?,
        };
        self.get(self.connection.last_insert_rowid())
    }

    /// Removes a project from the list. The folder is not touched.
    pub fn remove(&self, id: i64) -> Result<()> {
        let removed = self
            .connection
            .execute("DELETE FROM project WHERE id = ?1", [id])?;
        if removed == 0 {
            return Err(Error::ProjectNotFound(id));
        }
        Ok(())
    }
}

fn read_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Project> {
    let path = PathBuf::from(row.get::<_, String>(1)?);
    let name = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    let error = (!path.is_dir()).then(|| "the folder does not exist".to_owned());
    Ok(Project {
        id: row.get(0)?,
        name,
        path,
        added_at: row.get(2)?,
        error,
    })
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn adds_lists_and_removes_project_folders() {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let projects = Projects::open(data.path()).unwrap();

        let added = projects.add(folder.path().to_str().unwrap()).unwrap();

        assert_eq!(added.path, folder.path());
        assert_eq!(
            added.name,
            folder.path().file_name().unwrap().to_string_lossy()
        );
        assert_eq!(added.error, None);
        assert_eq!(projects.list().unwrap().len(), 1);
        projects.remove(added.id).unwrap();
        assert!(projects.list().unwrap().is_empty());
        assert!(folder.path().is_dir());
        assert!(matches!(
            projects.remove(added.id),
            Err(Error::ProjectNotFound(_))
        ));
    }

    #[test]
    fn refuses_a_missing_folder_and_a_duplicate() {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let projects = Projects::open(data.path()).unwrap();
        let file = folder.path().join("file.txt");
        fs::write(&file, "").unwrap();

        projects.add(folder.path().to_str().unwrap()).unwrap();

        for input in [
            folder.path().to_str().unwrap(),
            file.to_str().unwrap(),
            folder.path().join("missing").to_str().unwrap(),
            "",
        ] {
            assert!(projects.add(input).is_err(), "{input:?}");
        }
        assert_eq!(projects.list().unwrap().len(), 1);
    }

    #[test]
    fn reports_a_folder_that_was_moved() {
        let data = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let projects = Projects::open(data.path()).unwrap();
        let moving = folder.path().join("moving");
        fs::create_dir(&moving).unwrap();
        let added = projects.add(moving.to_str().unwrap()).unwrap();

        fs::remove_dir(&moving).unwrap();

        assert!(projects.get(added.id).unwrap().error.is_some());
    }
}
