use std::collections::HashMap;
use std::fs;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use thiserror::Error;

use crate::theme::{valid_app_theme, valid_category_color, DEFAULT_THEME};

pub const CATEGORY_EMOJIS: &[&str] = &[
    "🏠", "💼", "💰", "📚", "🛒", "🎯", "❤️", "💪", "🍽️", "✈️", "🎨", "🎵", "🌱", "🐾", "💻", "🔧",
    "📅", "⭐", "🎮", "🚗", "🏖️", "🎁", "☕", "📝", "🏥", "🎓", "📈", "🏦", "🧹", "🧺", "🛏️", "🚿",
    "🧘", "🏃", "🥗", "🍕", "🌍", "🌤️", "🌧️", "🌸", "🌳", "🐶", "🐱", "👶", "👪", "📱", "📷", "🔑",
];

pub fn valid_category_icon(icon: &str) -> bool {
    icon.is_empty() || CATEGORY_EMOJIS.contains(&icon)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Todo {
    pub id: i64,
    pub title: String,
    pub done: bool,
    pub category_id: i64,
    pub position: i64,
    /// Sub-category this todo is nested under. Positions are ordered among siblings
    /// sharing the same `(category_id, parent_id)`.
    pub parent_id: Option<i64>,
    /// True when this row is a checkable sub-category heading that can hold nested todos.
    /// Sub-categories always live at the top level of a category (one nesting level).
    pub is_group: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub color: String,
    pub position: i64,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("todo not found")]
    TodoNotFound,
    #[error("category not found")]
    CategoryNotFound,
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Storage(String),
}

/// In-memory view backed by SQLite. All SQLite writes succeed before the cached maps change.
pub struct MemoryDb {
    disk: Option<Connection>,
    todos: HashMap<i64, Todo>,
    categories: HashMap<i64, Category>,
    app_theme: String,
}

pub fn open_default_db() -> Result<MemoryDb, StoreError> {
    let root = dirs::config_dir()
        .ok_or_else(|| StoreError::Storage("find application data directory".into()))?;
    let dir = root.join("scrivi");
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(&dir)
            .map_err(|e| StoreError::Storage(format!("create application data directory: {e}")))?;
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(&dir)
            .map_err(|e| StoreError::Storage(format!("create application data directory: {e}")))?;
    }
    MemoryDb::open(dir.join("todos.sqlite"))
}

impl MemoryDb {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let disk = Connection::open(path.as_ref())
            .map_err(|e| StoreError::Storage(format!("open todo database: {e}")))?;
        Self::from_connection(disk)
    }

    pub fn open_in_memory() -> Result<Self, StoreError> {
        let disk = Connection::open_in_memory()
            .map_err(|e| StoreError::Storage(format!("open todo database: {e}")))?;
        Self::from_connection(disk)
    }

    fn from_connection(disk: Connection) -> Result<Self, StoreError> {
        let mut db = Self {
            disk: Some(disk),
            todos: HashMap::new(),
            categories: HashMap::new(),
            app_theme: String::new(),
        };
        db.load()?;
        Ok(db)
    }

    fn conn(&self) -> Result<&Connection, StoreError> {
        self.disk
            .as_ref()
            .ok_or_else(|| StoreError::Storage("database is closed".into()))
    }

    fn conn_mut(&mut self) -> Result<&mut Connection, StoreError> {
        self.disk
            .as_mut()
            .ok_or_else(|| StoreError::Storage("database is closed".into()))
    }

    pub fn close(&mut self) -> Result<(), StoreError> {
        if let Some(conn) = self.disk.take() {
            conn.close()
                .map_err(|(_, e)| StoreError::Storage(format!("close todo database: {e}")))?;
        }
        Ok(())
    }

    /// Test helper for installing SQLite triggers or inspecting raw database state.
    #[doc(hidden)]
    pub fn exec_sql_for_test(&self, sql: &str) -> Result<(), StoreError> {
        self.conn()?
            .execute_batch(sql)
            .map_err(|e| StoreError::Storage(format!("execute sql: {e}")))
    }

    fn load(&mut self) -> Result<(), StoreError> {
        let conn = self.conn()?;
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS categories (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 name TEXT NOT NULL COLLATE NOCASE UNIQUE,
                 icon TEXT NOT NULL DEFAULT ''
             );",
        )
        .map_err(|e| StoreError::Storage(format!("initialize category database: {e}")))?;

        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS todos (
                 id INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 done INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
                 category_id INTEGER REFERENCES categories(id)
             );",
        )
        .map_err(|e| StoreError::Storage(format!("initialize todo database: {e}")))?;

        self.migrate_categories()?;
        self.migrate_category_icons()?;
        self.migrate_appearance_and_order()?;
        self.migrate_subcategories()?;
        self.recover_uncategorized()?;
        self.load_categories()?;

        let (stored_theme, loaded_todos) = {
            let conn = self.conn()?;
            let stored_theme: Option<String> = conn
                .query_row("SELECT value FROM settings WHERE key = 'theme'", [], |row| {
                    row.get(0)
                })
                .optional()
                .map_err(|e| StoreError::Storage(format!("load theme: {e}")))?;

            let mut stmt = conn
                .prepare(
                    "SELECT id, title, done, COALESCE(category_id, 0), position, parent_id, is_group FROM todos",
                )
                .map_err(|e| StoreError::Storage(format!("load todos: {e}")))?;
            let rows = stmt
                .query_map([], |row| {
                    let done_int: i64 = row.get(2)?;
                    let group_int: i64 = row.get(6)?;
                    Ok(Todo {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        done: done_int != 0,
                        category_id: row.get(3)?,
                        position: row.get(4)?,
                        parent_id: row.get(5)?,
                        is_group: group_int != 0,
                    })
                })
                .map_err(|e| StoreError::Storage(format!("load todos: {e}")))?;

            let mut loaded_todos = Vec::new();
            for row in rows {
                loaded_todos.push(row.map_err(|e| StoreError::Storage(format!("read todo: {e}")))?);
            }
            (stored_theme, loaded_todos)
        };

        self.app_theme = match stored_theme {
            Some(t) if !t.is_empty() => t,
            _ => DEFAULT_THEME.to_string(),
        };
        if !valid_app_theme(&self.app_theme) {
            return Err(StoreError::Storage(format!(
                "load theme: unknown theme {:?}",
                self.app_theme
            )));
        }
        for item in loaded_todos {
            self.todos.insert(item.id, item);
        }
        Ok(())
    }

    fn table_has_column(conn: &Connection, table: &str, col: &str) -> Result<bool, StoreError> {
        let pragma = format!("PRAGMA table_info({table})");
        let mut stmt = conn
            .prepare(&pragma)
            .map_err(|e| StoreError::Storage(format!("inspect {table} schema: {e}")))?;
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .map_err(|e| StoreError::Storage(format!("inspect {table} schema: {e}")))?;
        for row in rows {
            let name = row.map_err(|e| StoreError::Storage(format!("read {table} schema: {e}")))?;
            if name == col {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn migrate_categories(&self) -> Result<(), StoreError> {
        let conn = self.conn()?;
        if !Self::table_has_column(conn, "todos", "category_id")? {
            conn.execute_batch(
                "ALTER TABLE todos ADD COLUMN category_id INTEGER REFERENCES categories(id)",
            )
            .map_err(|e| StoreError::Storage(format!("migrate todo categories: {e}")))?;
        }
        Ok(())
    }

    fn migrate_category_icons(&self) -> Result<(), StoreError> {
        let conn = self.conn()?;
        if !Self::table_has_column(conn, "categories", "icon")? {
            conn.execute_batch("ALTER TABLE categories ADD COLUMN icon TEXT NOT NULL DEFAULT ''")
                .map_err(|e| StoreError::Storage(format!("migrate category icons: {e}")))?;
        }
        Ok(())
    }

    fn migrate_appearance_and_order(&mut self) -> Result<(), StoreError> {
        let conn = self.conn_mut()?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::Storage(format!("begin appearance migration: {e}")))?;

        let columns = [
            ("categories", "position", "INTEGER NOT NULL DEFAULT 0"),
            ("categories", "color", "TEXT NOT NULL DEFAULT ''"),
            ("todos", "position", "INTEGER NOT NULL DEFAULT 0"),
        ];

        for (table, name, definition) in columns {
            if Self::table_has_column(&tx, table, name)? {
                continue;
            }
            tx.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {name} {definition}"))
                .map_err(|e| StoreError::Storage(format!("migrate {table} {name}: {e}")))?;
            if name == "position" {
                tx.execute_batch(&format!("UPDATE {table} SET position = id"))
                    .map_err(|e| StoreError::Storage(format!("initialize {table} order: {e}")))?;
            }
        }

        tx.execute_batch(
            "CREATE TABLE IF NOT EXISTS settings (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
        )
        .map_err(|e| StoreError::Storage(format!("initialize settings: {e}")))?;

        tx.commit()
            .map_err(|e| StoreError::Storage(format!("save appearance migration: {e}")))?;
        Ok(())
    }

    fn migrate_subcategories(&mut self) -> Result<(), StoreError> {
        let conn = self.conn_mut()?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::Storage(format!("begin sub-category migration: {e}")))?;
        let columns = [
            ("parent_id", "INTEGER REFERENCES todos(id)"),
            ("is_group", "INTEGER NOT NULL DEFAULT 0 CHECK (is_group IN (0, 1))"),
        ];
        for (name, definition) in columns {
            if Self::table_has_column(&tx, "todos", name)? {
                continue;
            }
            tx.execute_batch(&format!("ALTER TABLE todos ADD COLUMN {name} {definition}"))
                .map_err(|e| StoreError::Storage(format!("migrate todos {name}: {e}")))?;
        }
        tx.commit()
            .map_err(|e| StoreError::Storage(format!("save sub-category migration: {e}")))?;
        Ok(())
    }

    fn recover_uncategorized(&mut self) -> Result<(), StoreError> {
        let conn = self.conn_mut()?;
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM todos WHERE category_id IS NULL",
                [],
                |row| row.get(0),
            )
            .map_err(|e| StoreError::Storage(format!("check uncategorized todos: {e}")))?;
        if count == 0 {
            return Ok(());
        }

        let tx = conn
            .transaction()
            .map_err(|e| StoreError::Storage(format!("begin todo recovery: {e}")))?;

        let mut name = "Recovered todos".to_string();
        let mut suffix = 2usize;
        loop {
            let exists: bool = tx
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM categories WHERE name = ?1 COLLATE NOCASE)",
                    params![name],
                    |row| row.get(0),
                )
                .map_err(|e| StoreError::Storage(format!("find recovery category: {e}")))?;
            if !exists {
                break;
            }
            name = format!("Recovered todos {suffix}");
            suffix += 1;
        }

        tx.execute(
            "INSERT INTO categories (name, position) SELECT ?1, COALESCE(MAX(position), -1) + 1 FROM categories",
            params![name],
        )
        .map_err(|e| StoreError::Storage(format!("create recovery category: {e}")))?;

        let id = tx.last_insert_rowid();
        tx.execute(
            "UPDATE todos SET category_id = ?1 WHERE category_id IS NULL",
            params![id],
        )
        .map_err(|e| StoreError::Storage(format!("recover uncategorized todos: {e}")))?;

        tx.commit()
            .map_err(|e| StoreError::Storage(format!("save recovered todos: {e}")))?;
        Ok(())
    }

    fn load_categories(&mut self) -> Result<(), StoreError> {
        let loaded = {
            let conn = self.conn()?;
            let mut stmt = conn
                .prepare("SELECT id, name, icon, color, position FROM categories")
                .map_err(|e| StoreError::Storage(format!("load categories: {e}")))?;
            let rows = stmt
                .query_map([], |row| {
                    Ok(Category {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        icon: row.get(2)?,
                        color: row.get(3)?,
                        position: row.get(4)?,
                    })
                })
                .map_err(|e| StoreError::Storage(format!("load categories: {e}")))?;

            let mut items = Vec::new();
            for row in rows {
                let item = row.map_err(|e| StoreError::Storage(format!("read category: {e}")))?;
                if !valid_category_color(&item.color) {
                    return Err(StoreError::Storage(format!(
                        "read category {}: unknown color {:?}",
                        item.id, item.color
                    )));
                }
                items.push(item);
            }
            items
        };

        for item in loaded {
            self.categories.insert(item.id, item);
        }
        Ok(())
    }

    pub fn app_theme(&self) -> &str {
        &self.app_theme
    }

    pub fn category(&self, id: i64) -> Option<Category> {
        self.categories.get(&id).cloned()
    }

    pub fn todo(&self, id: i64) -> Option<Todo> {
        self.todos.get(&id).cloned()
    }

    pub fn add_in_category(&mut self, title: &str, category_id: i64) -> Result<i64, StoreError> {
        self.insert_todo(title, category_id, None, false)
    }

    /// Adds a checkable sub-category heading at the end of a category's top-level list.
    pub fn add_subcategory(&mut self, title: &str, category_id: i64) -> Result<i64, StoreError> {
        self.insert_todo(title, category_id, None, true)
    }

    /// Adds a todo nested at the end of the given sub-category.
    pub fn add_in_subcategory(&mut self, title: &str, group_id: i64) -> Result<i64, StoreError> {
        let group = self.group(group_id)?;
        self.insert_todo(title, group.category_id, Some(group_id), false)
    }

    fn group(&self, id: i64) -> Result<&Todo, StoreError> {
        match self.todos.get(&id) {
            Some(item) if item.is_group => Ok(item),
            Some(_) => Err(StoreError::Validation(
                "todos can only be nested under a sub-category".into(),
            )),
            None => Err(StoreError::TodoNotFound),
        }
    }

    fn insert_todo(
        &mut self,
        title: &str,
        category_id: i64,
        parent_id: Option<i64>,
        is_group: bool,
    ) -> Result<i64, StoreError> {
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(StoreError::Validation(if is_group {
                "enter a sub-category name first".into()
            } else {
                "enter a todo first".into()
            }));
        }
        if !self.categories.contains_key(&category_id) {
            return Err(StoreError::CategoryNotFound);
        }
        let position = self
            .todos
            .values()
            .filter(|t| t.category_id == category_id && t.parent_id == parent_id)
            .map(|t| t.position + 1)
            .max()
            .unwrap_or(0);

        let uncheck_parent = parent_id.filter(|p| self.todos.get(p).is_some_and(|g| g.done));
        let id = if let Some(parent) = uncheck_parent {
            let conn = self.conn_mut()?;
            let tx = conn
                .transaction()
                .map_err(|e| StoreError::Storage(format!("begin todo save: {e}")))?;
            tx.execute(
                "INSERT INTO todos (title, category_id, position, parent_id, is_group) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![title, category_id, position, parent_id, is_group],
            )
            .map_err(|e| StoreError::Storage(format!("save todo: {e}")))?;
            let id = tx.last_insert_rowid();
            tx.execute("UPDATE todos SET done = 0 WHERE id = ?1", params![parent])
                .map_err(|e| StoreError::Storage(format!("save completion: {e}")))?;
            tx.commit()
                .map_err(|e| StoreError::Storage(format!("save todo: {e}")))?;
            if let Some(g) = self.todos.get_mut(&parent) {
                g.done = false;
            }
            id
        } else {
            let conn = self.conn()?;
            conn.execute(
                "INSERT INTO todos (title, category_id, position, parent_id, is_group) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![title, category_id, position, parent_id, is_group],
            )
            .map_err(|e| StoreError::Storage(format!("save todo: {e}")))?;
            conn.last_insert_rowid()
        };
        self.todos.insert(
            id,
            Todo {
                id,
                title,
                done: false,
                category_id,
                position,
                parent_id,
                is_group,
            },
        );
        Ok(id)
    }

    pub fn add_category(&mut self, name: &str) -> Result<i64, StoreError> {
        self.add_category_with_icon(name, "")
    }

    pub fn add_category_with_icon(&mut self, name: &str, icon: &str) -> Result<i64, StoreError> {
        if !valid_category_icon(icon) {
            return Err(StoreError::Validation(
                "choose a category emoji from the picker".into(),
            ));
        }
        let name = name.trim().to_string();
        if name.is_empty() {
            return Err(StoreError::Validation(
                "enter a category name first".into(),
            ));
        }
        let lower = name.to_lowercase();
        if self
            .categories
            .values()
            .any(|item| item.name.to_lowercase() == lower)
        {
            return Err(StoreError::Validation(
                "a category with that name already exists".into(),
            ));
        }
        let position = self
            .categories
            .values()
            .map(|c| c.position + 1)
            .max()
            .unwrap_or(0);

        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO categories (name, icon, position) VALUES (?1, ?2, ?3)",
            params![name, icon, position],
        )
        .map_err(|e| StoreError::Storage(format!("save category: {e}")))?;
        let id = conn.last_insert_rowid();
        self.categories.insert(
            id,
            Category {
                id,
                name,
                icon: icon.to_string(),
                color: String::new(),
                position,
            },
        );
        Ok(id)
    }

    pub fn set_category_icon(&mut self, id: i64, icon: &str) -> Result<(), StoreError> {
        if !self.categories.contains_key(&id) {
            return Err(StoreError::CategoryNotFound);
        }
        if !valid_category_icon(icon) {
            return Err(StoreError::Validation(
                "choose a category emoji from the picker".into(),
            ));
        }
        self.conn()?
            .execute(
                "UPDATE categories SET icon = ?1 WHERE id = ?2",
                params![icon, id],
            )
            .map_err(|e| StoreError::Storage(format!("save category icon: {e}")))?;
        if let Some(item) = self.categories.get_mut(&id) {
            item.icon = icon.to_string();
        }
        Ok(())
    }

    pub fn set_category_color(&mut self, id: i64, color: &str) -> Result<(), StoreError> {
        if !self.categories.contains_key(&id) {
            return Err(StoreError::CategoryNotFound);
        }
        if !valid_category_color(color) {
            return Err(StoreError::Validation(
                "choose a valid category color".into(),
            ));
        }
        self.conn()?
            .execute(
                "UPDATE categories SET color = ?1 WHERE id = ?2",
                params![color, id],
            )
            .map_err(|e| StoreError::Storage(format!("save category color: {e}")))?;
        if let Some(item) = self.categories.get_mut(&id) {
            item.color = color.to_string();
        }
        Ok(())
    }

    pub fn set_app_theme(&mut self, name: &str) -> Result<(), StoreError> {
        if !valid_app_theme(name) {
            return Err(StoreError::Validation("choose a valid theme".into()));
        }
        self.conn()?
            .execute(
                "INSERT INTO settings (key, value) VALUES ('theme', ?1) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![name],
            )
            .map_err(|e| StoreError::Storage(format!("save theme: {e}")))?;
        self.app_theme = name.to_string();
        Ok(())
    }

    pub fn list_categories(&self) -> Vec<Category> {
        let mut items: Vec<Category> = self.categories.values().cloned().collect();
        items.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.id.cmp(&b.id)));
        items
    }

    pub fn delete_category(&mut self, id: i64) -> Result<(), StoreError> {
        if !self.categories.contains_key(&id) {
            return Err(StoreError::CategoryNotFound);
        }
        let conn = self.conn_mut()?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::Storage(format!("begin category deletion: {e}")))?;
        tx.execute(
            "DELETE FROM todos WHERE category_id = ?1 AND parent_id IS NOT NULL",
            params![id],
        )
        .map_err(|e| StoreError::Storage(format!("delete category todos: {e}")))?;
        tx.execute("DELETE FROM todos WHERE category_id = ?1", params![id])
            .map_err(|e| StoreError::Storage(format!("delete category todos: {e}")))?;
        tx.execute("DELETE FROM categories WHERE id = ?1", params![id])
            .map_err(|e| StoreError::Storage(format!("delete category: {e}")))?;
        tx.commit()
            .map_err(|e| StoreError::Storage(format!("save category deletion: {e}")))?;

        self.categories.remove(&id);
        self.todos.retain(|_, item| item.category_id != id);
        Ok(())
    }

    pub fn set_title(&mut self, id: i64, title: &str) -> Result<(), StoreError> {
        let Some(existing) = self.todos.get(&id) else {
            return Err(StoreError::TodoNotFound);
        };
        let title = title.trim().to_string();
        if title.is_empty() {
            return Err(StoreError::Validation(if existing.is_group {
                "enter a sub-category name first".into()
            } else {
                "enter a todo first".into()
            }));
        }
        self.conn()?
            .execute(
                "UPDATE todos SET title = ?1 WHERE id = ?2",
                params![title, id],
            )
            .map_err(|e| StoreError::Storage(format!("save todo title: {e}")))?;
        if let Some(item) = self.todos.get_mut(&id) {
            item.title = title;
        }
        Ok(())
    }

    pub fn list(&self) -> Vec<Todo> {
        let mut todos: Vec<Todo> = self.todos.values().cloned().collect();
        todos.sort_by_key(|item| item.id);
        todos
    }

    /// Top-level todos and sub-categories of a category, in display order.
    pub fn list_category_todos(&self, category_id: i64) -> Vec<Todo> {
        self.list_siblings(category_id, None)
    }

    /// Todos nested under a sub-category, in display order.
    pub fn list_subcategory_todos(&self, group_id: i64) -> Vec<Todo> {
        match self.todos.get(&group_id) {
            Some(group) => self.list_siblings(group.category_id, Some(group_id)),
            None => Vec::new(),
        }
    }

    fn list_siblings(&self, category_id: i64, parent_id: Option<i64>) -> Vec<Todo> {
        let mut items: Vec<Todo> = self
            .todos
            .values()
            .filter(|item| item.category_id == category_id && item.parent_id == parent_id)
            .cloned()
            .collect();
        items.sort_by(|a, b| a.position.cmp(&b.position).then_with(|| a.id.cmp(&b.id)));
        items
    }

    pub fn move_category(&mut self, id: i64, index: isize) -> Result<(), StoreError> {
        let Some(moving) = self.categories.get(&id).cloned() else {
            return Err(StoreError::CategoryNotFound);
        };
        let groups = self.list_categories();
        let mut remaining: Vec<Category> = groups.into_iter().filter(|g| g.id != id).collect();
        if index < 0 || (index as usize) > remaining.len() {
            return Err(StoreError::Validation("invalid category position".into()));
        }
        remaining.insert(index as usize, moving);

        let conn = self.conn_mut()?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::Storage(format!("begin category reorder: {e}")))?;
        for (pos, group) in remaining.iter().enumerate() {
            tx.execute(
                "UPDATE categories SET position = ?1 WHERE id = ?2",
                params![pos as i64, group.id],
            )
            .map_err(|e| StoreError::Storage(format!("save category order: {e}")))?;
        }
        tx.commit()
            .map_err(|e| StoreError::Storage(format!("save category order: {e}")))?;

        for (pos, mut group) in remaining.into_iter().enumerate() {
            group.position = pos as i64;
            self.categories.insert(group.id, group);
        }
        Ok(())
    }

    /// Moves a todo (or sub-category) to `index` within a category's top-level list.
    pub fn move_todo(&mut self, id: i64, category_id: i64, index: isize) -> Result<(), StoreError> {
        self.move_todo_to(id, category_id, None, index)
    }

    /// Moves a todo to `index` among the siblings under `parent_id` (a sub-category in
    /// `category_id`), or among the category's top-level items when `parent_id` is `None`.
    /// Sub-categories carry their nested todos with them and cannot themselves be nested.
    pub fn move_todo_to(
        &mut self,
        id: i64,
        category_id: i64,
        parent_id: Option<i64>,
        index: isize,
    ) -> Result<(), StoreError> {
        let Some(mut item) = self.todos.get(&id).cloned() else {
            return Err(StoreError::TodoNotFound);
        };
        if !self.categories.contains_key(&category_id) {
            return Err(StoreError::CategoryNotFound);
        }
        if let Some(parent) = parent_id {
            if item.is_group || parent == id {
                return Err(StoreError::Validation(
                    "sub-categories cannot be nested".into(),
                ));
            }
            if self.group(parent)?.category_id != category_id {
                return Err(StoreError::Validation(
                    "sub-category belongs to another category".into(),
                ));
            }
        }
        let mut remaining: Vec<Todo> = self
            .list_siblings(category_id, parent_id)
            .into_iter()
            .filter(|t| t.id != id)
            .collect();
        if index < 0 || (index as usize) > remaining.len() {
            return Err(StoreError::Validation("invalid todo position".into()));
        }
        let children: Vec<i64> = if item.is_group {
            self.todos
                .values()
                .filter(|t| t.parent_id == Some(id))
                .map(|t| t.id)
                .collect()
        } else {
            Vec::new()
        };
        let old_parent = item.parent_id;
        let is_group = item.is_group;
        item.category_id = category_id;
        item.parent_id = parent_id;
        remaining.insert(index as usize, item);

        let mut parent_done_updates: Vec<(i64, bool)> = Vec::new();
        if !is_group {
            if let Some(p) = parent_id {
                let all_done = remaining.iter().all(|t| t.done);
                if self.todos.get(&p).is_some_and(|g| g.done != all_done) {
                    parent_done_updates.push((p, all_done));
                }
            }
            if let Some(old_p) = old_parent {
                if old_parent != parent_id {
                    let old_children: Vec<&Todo> = self
                        .todos
                        .values()
                        .filter(|t| t.parent_id == Some(old_p) && t.id != id)
                        .collect();
                    if !old_children.is_empty() {
                        let all_done = old_children.iter().all(|t| t.done);
                        if self.todos.get(&old_p).is_some_and(|g| g.done != all_done) {
                            parent_done_updates.push((old_p, all_done));
                        }
                    }
                }
            }
        }

        let conn = self.conn_mut()?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::Storage(format!("begin todo move: {e}")))?;
        for (pos, target) in remaining.iter().enumerate() {
            tx.execute(
                "UPDATE todos SET category_id = ?1, parent_id = ?2, position = ?3 WHERE id = ?4",
                params![category_id, parent_id, pos as i64, target.id],
            )
            .map_err(|e| StoreError::Storage(format!("save todo order: {e}")))?;
        }
        if !children.is_empty() {
            tx.execute(
                "UPDATE todos SET category_id = ?1 WHERE parent_id = ?2",
                params![category_id, id],
            )
            .map_err(|e| StoreError::Storage(format!("move sub-category todos: {e}")))?;
        }
        for (parent, done) in &parent_done_updates {
            tx.execute(
                "UPDATE todos SET done = ?1 WHERE id = ?2",
                params![done, parent],
            )
            .map_err(|e| StoreError::Storage(format!("save completion: {e}")))?;
        }
        tx.commit()
            .map_err(|e| StoreError::Storage(format!("save todo order: {e}")))?;

        for (pos, mut target) in remaining.into_iter().enumerate() {
            target.category_id = category_id;
            target.parent_id = parent_id;
            target.position = pos as i64;
            self.todos.insert(target.id, target);
        }
        for child in children {
            if let Some(t) = self.todos.get_mut(&child) {
                t.category_id = category_id;
            }
        }
        for (parent, done) in parent_done_updates {
            if let Some(t) = self.todos.get_mut(&parent) {
                t.done = done;
            }
        }
        Ok(())
    }

    /// Sets completion. Toggling a sub-category applies to all of its todos; toggling a
    /// nested todo checks the sub-category once every sibling is done (and unchecks it
    /// otherwise).
    pub fn set_done(&mut self, id: i64, done: bool) -> Result<(), StoreError> {
        let Some(item) = self.todos.get(&id).cloned() else {
            return Err(StoreError::TodoNotFound);
        };
        let mut updates: Vec<(i64, bool)> = vec![(id, done)];
        if item.is_group {
            updates.extend(
                self.todos
                    .values()
                    .filter(|t| t.parent_id == Some(id))
                    .map(|t| (t.id, done)),
            );
        } else if let Some(parent) = item.parent_id {
            let all_done = self
                .todos
                .values()
                .filter(|t| t.parent_id == Some(parent))
                .all(|t| if t.id == id { done } else { t.done });
            if self.todos.get(&parent).is_some_and(|g| g.done != all_done) {
                updates.push((parent, all_done));
            }
        }

        let conn = self.conn_mut()?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::Storage(format!("begin completion: {e}")))?;
        for (target, value) in &updates {
            tx.execute(
                "UPDATE todos SET done = ?1 WHERE id = ?2",
                params![value, target],
            )
            .map_err(|e| StoreError::Storage(format!("save completion: {e}")))?;
        }
        tx.commit()
            .map_err(|e| StoreError::Storage(format!("save completion: {e}")))?;

        for (target, value) in updates {
            if let Some(t) = self.todos.get_mut(&target) {
                t.done = value;
            }
        }
        Ok(())
    }

    /// Deletes a todo. Deleting a sub-category also deletes the todos nested under it.
    pub fn delete(&mut self, id: i64) -> Result<(), StoreError> {
        let Some(old_parent) = self.todos.get(&id).map(|t| t.parent_id) else {
            return Err(StoreError::TodoNotFound);
        };
        let parent_done_update = old_parent.and_then(|p| {
            let remaining: Vec<&Todo> = self
                .todos
                .values()
                .filter(|t| t.parent_id == Some(p) && t.id != id)
                .collect();
            if remaining.is_empty() {
                return None;
            }
            let all_done = remaining.iter().all(|t| t.done);
            self.todos
                .get(&p)
                .filter(|g| g.done != all_done)
                .map(|_| (p, all_done))
        });

        let conn = self.conn_mut()?;
        let tx = conn
            .transaction()
            .map_err(|e| StoreError::Storage(format!("begin todo deletion: {e}")))?;
        tx.execute("DELETE FROM todos WHERE parent_id = ?1", params![id])
            .map_err(|e| StoreError::Storage(format!("delete sub-category todos: {e}")))?;
        tx.execute("DELETE FROM todos WHERE id = ?1", params![id])
            .map_err(|e| StoreError::Storage(format!("delete todo: {e}")))?;
        if let Some((parent, done)) = parent_done_update {
            tx.execute(
                "UPDATE todos SET done = ?1 WHERE id = ?2",
                params![done, parent],
            )
            .map_err(|e| StoreError::Storage(format!("save completion: {e}")))?;
        }
        tx.commit()
            .map_err(|e| StoreError::Storage(format!("save todo deletion: {e}")))?;
        self.todos
            .retain(|_, item| item.id != id && item.parent_id != Some(id));
        if let Some((parent, done)) = parent_done_update {
            if let Some(g) = self.todos.get_mut(&parent) {
                g.done = done;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn add_test_todo(db: &mut MemoryDb, title: &str) -> Result<i64, StoreError> {
        let mut groups = db.list_categories();
        if groups.is_empty() {
            db.add_category("Test")?;
            groups = db.list_categories();
        }
        db.add_in_category(title, groups[0].id)
    }

    #[test]
    fn test_categories_persist_and_delete_their_todos() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let mut db = MemoryDb::open(&path).unwrap();
        for name in ["  Life  ", "Money", "Work"] {
            db.add_category(name).unwrap();
        }
        let groups = db.list_categories();
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].name, "Life");
        for (i, group) in groups.iter().enumerate() {
            if i > 0 {
                assert!(group.id > groups[i - 1].id);
            }
            db.add_in_category(&format!("{} task", group.name), group.id)
                .unwrap();
        }
        let items = db.list();
        db.set_done(items[1].id, true).unwrap();

        let mut reopened = MemoryDb::open(&path).unwrap();
        assert_eq!(reopened.list_categories(), groups);
        assert_eq!(reopened.list(), db.list());
        reopened.close().unwrap();

        db.delete_category(groups[0].id).unwrap();
        assert_eq!(db.list_categories().len(), 2);
        let items = db.list();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Money task");
        assert!(items[0].done);
        assert_eq!(items[0].category_id, groups[1].id);
        assert_eq!(items[1].category_id, groups[2].id);

        let mut reopened = MemoryDb::open(&path).unwrap();
        assert_eq!(reopened.list_categories(), db.list_categories());
        assert_eq!(reopened.list(), items);
        reopened.close().unwrap();
        db.close().unwrap();
    }

    #[test]
    fn test_invalid_category_operations() {
        let mut db = MemoryDb::open_in_memory().unwrap();
        db.add_category("Work").unwrap();
        for name in ["", " \t\n ", " work "] {
            assert!(db.add_category(name).is_err());
        }
        for id in [0, 42] {
            assert!(matches!(
                db.delete_category(id),
                Err(StoreError::CategoryNotFound)
            ));
        }
        assert!(matches!(
            db.add_in_category("Task", 42),
            Err(StoreError::CategoryNotFound)
        ));
        assert!(matches!(
            db.add_in_category("Task", 0),
            Err(StoreError::CategoryNotFound)
        ));
        assert_eq!(db.list_categories().len(), 1);
        assert!(db.list().is_empty());
    }

    #[test]
    fn test_category_icons_persist_and_preserve_cache_on_failure() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let mut db = MemoryDb::open(&path).unwrap();
        db.add_category_with_icon("Life", "🏠").unwrap();
        let id = db.list_categories()[0].id;

        let check_persisted = |db: &MemoryDb, icon: &str| {
            let mut reopened = MemoryDb::open(&path).unwrap();
            assert_eq!(reopened.category(id).unwrap().icon, icon);
            assert_eq!(db.category(id).unwrap().icon, icon);
            reopened.close().unwrap();
        };

        check_persisted(&db, "🏠");
        db.set_category_icon(id, "⭐").unwrap();
        check_persisted(&db, "⭐");

        assert!(matches!(
            db.set_category_icon(42, "⭐"),
            Err(StoreError::CategoryNotFound)
        ));
        assert!(db.set_category_icon(id, "not an emoji").is_err());
        assert!(db.add_category_with_icon("Invalid", "not an emoji").is_err());

        db.exec_sql_for_test(
            "CREATE TRIGGER prevent_icon_update BEFORE UPDATE OF icon ON categories
             BEGIN SELECT RAISE(ABORT, 'update blocked'); END;",
        )
        .unwrap();
        assert!(db.set_category_icon(id, "💼").is_err());
        check_persisted(&db, "⭐");

        db.exec_sql_for_test("DROP TRIGGER prevent_icon_update;")
            .unwrap();
        db.set_category_icon(id, "").unwrap();
        check_persisted(&db, "");
        db.close().unwrap();
    }

    #[test]
    fn test_order_moves_and_themes_persist() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let mut db = MemoryDb::open(&path).unwrap();
        for name in ["Life", "Work", "Home"] {
            db.add_category(name).unwrap();
        }
        let groups = db.list_categories();
        for title in ["A", "B", "C"] {
            db.add_in_category(title, groups[0].id).unwrap();
        }
        db.add_in_category("D", groups[1].id).unwrap();
        let items = db.list();
        db.set_done(items[1].id, true).unwrap();
        db.move_category(groups[2].id, 0).unwrap();
        db.move_todo(items[2].id, groups[0].id, 0).unwrap();
        db.move_todo(items[1].id, groups[1].id, 0).unwrap();
        db.set_app_theme("Ocean").unwrap();
        db.set_category_color(groups[1].id, "Rose").unwrap();

        let got = db.list_categories();
        assert_eq!(got[0].name, "Home");
        assert_eq!(got[1].name, "Life");
        assert_eq!(got[2].name, "Work");

        let life = db.list_category_todos(groups[0].id);
        let work = db.list_category_todos(groups[1].id);
        assert_eq!(life.len(), 2);
        assert_eq!(life[0].title, "C");
        assert_eq!(life[1].title, "A");
        assert_eq!(work.len(), 2);
        assert_eq!(work[0].title, "B");
        assert!(work[0].done);
        assert_eq!(work[1].title, "D");

        let mut reopened = MemoryDb::open(&path).unwrap();
        assert_eq!(db.list_categories(), reopened.list_categories());
        assert_eq!(life, reopened.list_category_todos(groups[0].id));
        assert_eq!(work, reopened.list_category_todos(groups[1].id));
        assert_eq!(reopened.app_theme(), "Ocean");
        assert_eq!(reopened.category(groups[1].id).unwrap().color, "Rose");

        reopened.add_in_category("E", groups[1].id).unwrap();
        let work_after = reopened.list_category_todos(groups[1].id);
        assert_eq!(work_after.last().unwrap().title, "E");
        reopened.close().unwrap();
        db.close().unwrap();
    }

    #[test]
    fn test_order_and_theme_failures_preserve_state() {
        let mut db = MemoryDb::open_in_memory().unwrap();
        for name in ["Life", "Work"] {
            db.add_category(name).unwrap();
        }
        let groups = db.list_categories();
        for title in ["A", "B"] {
            db.add_in_category(title, groups[0].id).unwrap();
        }
        let items = db.list();

        assert!(matches!(
            db.move_todo(items[0].id, 42, 0),
            Err(StoreError::CategoryNotFound)
        ));
        assert!(db.move_todo(items[0].id, groups[1].id, 2).is_err());
        assert!(db.move_category(groups[0].id, -1).is_err());
        assert!(db.set_app_theme("unknown").is_err());
        assert!(db.set_category_color(groups[0].id, "unknown").is_err());

        db.exec_sql_for_test(
            "CREATE TRIGGER prevent_reorder BEFORE UPDATE OF position ON todos
             WHEN OLD.id = 1 BEGIN SELECT RAISE(ABORT, 'reorder blocked'); END;
             CREATE TRIGGER prevent_category_reorder BEFORE UPDATE OF position ON categories
             WHEN OLD.id = 1 BEGIN SELECT RAISE(ABORT, 'reorder blocked'); END;",
        )
        .unwrap();

        assert!(db.move_todo(items[1].id, groups[0].id, 0).is_err());
        assert!(db.move_category(groups[1].id, 0).is_err());
        assert_eq!(db.list(), items);
        assert_eq!(db.list_categories(), groups);
        assert_eq!(db.app_theme(), DEFAULT_THEME);

        let position: i64 = db
            .conn()
            .unwrap()
            .query_row(
                "SELECT position FROM todos WHERE id = ?1",
                params![items[1].id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(position, items[1].position);
    }

    #[test]
    fn test_existing_categories_migrate_without_losing_data() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let legacy = Connection::open(&path).unwrap();
        legacy
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 CREATE TABLE categories (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL COLLATE NOCASE UNIQUE);
                 CREATE TABLE todos (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL,
                     done INTEGER NOT NULL DEFAULT 0, category_id INTEGER REFERENCES categories(id));
                 INSERT INTO categories (id, name) VALUES (1, 'Work');
                 INSERT INTO todos (title, done, category_id) VALUES ('Keep', 1, 1);",
            )
            .unwrap();
        legacy.close().unwrap();

        let mut db = MemoryDb::open(&path).unwrap();
        assert_eq!(
            db.category(1).unwrap(),
            Category {
                id: 1,
                name: "Work".into(),
                icon: String::new(),
                color: String::new(),
                position: 1,
            }
        );
        assert_eq!(
            db.list(),
            vec![Todo {
                id: 1,
                title: "Keep".into(),
                done: true,
                category_id: 1,
                position: 1,
                parent_id: None,
                is_group: false,
            }]
        );
        db.set_category_icon(1, "💼").unwrap();
        db.close().unwrap();
    }

    #[test]
    fn test_category_deletion_rolls_back_on_failure() {
        let mut db = MemoryDb::open_in_memory().unwrap();
        db.add_category("Work").unwrap();
        let id = db.list_categories()[0].id;
        db.add_in_category("Keep", id).unwrap();
        let expected_todos = db.list();
        let expected_categories = db.list_categories();

        db.exec_sql_for_test(
            "CREATE TRIGGER prevent_category_delete BEFORE DELETE ON categories
             BEGIN SELECT RAISE(ABORT, 'deletion blocked'); END;",
        )
        .unwrap();
        assert!(db.delete_category(id).is_err());
        assert_eq!(db.list(), expected_todos);
        assert_eq!(db.list_categories(), expected_categories);

        let saved_category: i64 = db
            .conn()
            .unwrap()
            .query_row(
                "SELECT category_id FROM todos WHERE id = ?1",
                params![expected_todos[0].id],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(saved_category, id);

        db.close().unwrap();
        assert!(db.add_category("Unsaved").is_err());
        assert!(db.add_in_category("Unsaved", id).is_err());
        assert_eq!(db.list(), expected_todos);
        assert_eq!(db.list_categories(), expected_categories);
    }

    #[test]
    fn test_existing_todos_migrate_to_recovery_category() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let legacy = Connection::open(&path).unwrap();
        legacy
            .execute_batch(
                "CREATE TABLE todos (
                     id INTEGER PRIMARY KEY AUTOINCREMENT,
                     title TEXT NOT NULL,
                     done INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1))
                 );
                 INSERT INTO todos (title, done) VALUES ('Keep', 0), ('Complete', 1);",
            )
            .unwrap();
        legacy.close().unwrap();

        let mut db = MemoryDb::open(&path).unwrap();
        let groups = db.list_categories();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].name, "Recovered todos");
        let expected = vec![
            Todo {
                id: 1,
                title: "Keep".into(),
                done: false,
                category_id: groups[0].id,
                position: 1,
                parent_id: None,
                is_group: false,
            },
            Todo {
                id: 2,
                title: "Complete".into(),
                done: true,
                category_id: groups[0].id,
                position: 2,
                parent_id: None,
                is_group: false,
            },
        ];
        assert_eq!(db.list(), expected);

        db.add_category("Work").unwrap();
        let work_id = db.list_categories()[1].id;
        db.add_in_category("New", work_id).unwrap();

        let mut reopened = MemoryDb::open(&path).unwrap();
        assert_eq!(reopened.list(), db.list());
        reopened.close().unwrap();
        db.close().unwrap();
    }

    #[test]
    fn test_recovery_avoids_existing_category_names() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let mut db = MemoryDb::open(&path).unwrap();
        db.add_category("Recovered todos").unwrap();
        db.exec_sql_for_test("INSERT INTO todos (title) VALUES ('Legacy');")
            .unwrap();
        db.close().unwrap();

        let mut db = MemoryDb::open(&path).unwrap();
        let groups = db.list_categories();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[1].name, "Recovered todos 2");
        assert_eq!(db.list()[0].category_id, groups[1].id);
        db.close().unwrap();
    }

    #[test]
    fn test_recovery_rolls_back_on_failure() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let mut db = MemoryDb::open(&path).unwrap();
        db.exec_sql_for_test(
            "INSERT INTO todos (title) VALUES ('Legacy');
             CREATE TRIGGER prevent_recovery BEFORE UPDATE OF category_id ON todos
             BEGIN SELECT RAISE(ABORT, 'recovery blocked'); END;",
        )
        .unwrap();
        db.close().unwrap();

        assert!(MemoryDb::open(&path).is_err());

        let disk = Connection::open(&path).unwrap();
        let categories: i64 = disk
            .query_row("SELECT COUNT(*) FROM categories", [], |row| row.get(0))
            .unwrap();
        let unassigned: i64 = disk
            .query_row(
                "SELECT COUNT(*) FROM todos WHERE category_id IS NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(categories, 0);
        assert_eq!(unassigned, 1);
        disk.close().unwrap();
    }

    #[test]
    fn test_edit_todo_persists_and_preserves_state_on_failure() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let mut db = MemoryDb::open(&path).unwrap();
        add_test_todo(&mut db, "Original").unwrap();
        let id = db.list()[0].id;
        db.set_done(id, true).unwrap();
        db.set_title(id, "  Edited  ").unwrap();

        let expected = db.list();
        assert_eq!(expected[0].title, "Edited");
        assert!(expected[0].done);

        let mut reopened = MemoryDb::open(&path).unwrap();
        assert_eq!(reopened.list(), expected);

        assert!(db.set_title(id, " \t ").is_err());
        assert!(matches!(
            db.set_title(42, "Missing"),
            Err(StoreError::TodoNotFound)
        ));

        db.exec_sql_for_test(
            "CREATE TRIGGER prevent_title_update BEFORE UPDATE OF title ON todos
             BEGIN SELECT RAISE(ABORT, 'update blocked'); END;",
        )
        .unwrap();
        assert!(db.set_title(id, "Unsaved").is_err());

        let saved_title: String = reopened
            .conn()
            .unwrap()
            .query_row("SELECT title FROM todos WHERE id = ?1", params![id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(db.list(), expected);
        assert_eq!(saved_title, expected[0].title);
        reopened.close().unwrap();
        db.close().unwrap();
    }

    #[test]
    fn test_todo_lifecycle() {
        let mut db = MemoryDb::open_in_memory().unwrap();
        add_test_todo(&mut db, "  Buy milk  ").unwrap();
        add_test_todo(&mut db, "Read a book").unwrap();
        let items = db.list();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].title, "Buy milk");
        assert!(!items[0].done);
        assert!(items[0].id < items[1].id);

        let id = items[0].id;
        db.set_done(id, true).unwrap();
        assert!(db.list()[0].done);
        db.set_done(id, false).unwrap();
        assert!(!db.list()[0].done);

        db.delete(id).unwrap();
        let remaining = db.list();
        assert_eq!(remaining.len(), 1);
        assert_eq!(remaining[0].title, "Read a book");

        add_test_todo(&mut db, "Third todo").unwrap();
        assert!(db.list()[1].id > items[1].id);
        assert!(MemoryDb::open_in_memory().unwrap().list().is_empty());
    }

    #[test]
    fn test_invalid_operations() {
        let mut db = MemoryDb::open_in_memory().unwrap();
        for title in ["", " \t\n "] {
            assert!(add_test_todo(&mut db, title).is_err());
        }
        assert!(db.list().is_empty());
        assert!(matches!(db.set_done(42, true), Err(StoreError::TodoNotFound)));
        assert!(matches!(db.delete(42), Err(StoreError::TodoNotFound)));
    }

    #[test]
    fn test_list_returns_copies() {
        let mut db = MemoryDb::open_in_memory().unwrap();
        add_test_todo(&mut db, "Original").unwrap();
        let mut items = db.list();
        items[0].title = "Changed".into();
        assert_eq!(db.list()[0].title, "Original");
    }

    #[test]
    fn test_persistence() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let mut db = MemoryDb::open(&path).unwrap();
        for title in ["Keep", "Complete", "Delete"] {
            add_test_todo(&mut db, title).unwrap();
        }
        let items = db.list();
        db.set_done(items[1].id, true).unwrap();
        db.delete(items[2].id).unwrap();
        let expected = db.list();

        let mut reopened = MemoryDb::open(&path).unwrap();
        assert_eq!(reopened.list(), expected);
        reopened.close().unwrap();
        db.close().unwrap();

        let mut reopened = MemoryDb::open(&path).unwrap();
        assert_eq!(reopened.list(), expected);
        add_test_todo(&mut reopened, "New").unwrap();
        assert!(reopened.list()[2].id > items[2].id);
        reopened.close().unwrap();
    }

    #[test]
    fn test_failed_writes_preserve_memory() {
        let mut db = MemoryDb::open_in_memory().unwrap();
        add_test_todo(&mut db, "Keep").unwrap();
        let expected = db.list();
        db.close().unwrap();

        assert!(add_test_todo(&mut db, "Unsaved").is_err());
        assert_eq!(db.list(), expected);

        assert!(db.set_done(expected[0].id, true).is_err());
        assert_eq!(db.list(), expected);

        assert!(db.delete(expected[0].id).is_err());
        assert_eq!(db.list(), expected);
    }

    #[test]
    fn test_invalid_database_fails_to_open() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        fs::write(&path, b"not a SQLite database").unwrap();
        assert!(MemoryDb::open(&path).is_err());
    }

    fn titles(items: &[Todo]) -> Vec<&str> {
        items.iter().map(|t| t.title.as_str()).collect()
    }

    #[test]
    fn test_subcategories_nest_persist_and_move() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("todos.sqlite");
        let mut db = MemoryDb::open(&path).unwrap();
        let life = db.add_category("Life").unwrap();
        let work = db.add_category("Work").unwrap();
        db.add_in_category("Call mom", life).unwrap();
        let buy = db.add_subcategory("  To buy ", life).unwrap();
        let milk = db.add_in_subcategory("Milk", buy).unwrap();
        let eggs = db.add_in_subcategory("Eggs", buy).unwrap();
        let walk = db.add_in_category("Walk", life).unwrap();

        assert_eq!(titles(&db.list_category_todos(life)), ["Call mom", "To buy", "Walk"]);
        assert!(db.todo(buy).unwrap().is_group);
        assert_eq!(titles(&db.list_subcategory_todos(buy)), ["Milk", "Eggs"]);
        assert_eq!(db.todo(milk).unwrap().parent_id, Some(buy));

        // Invalid nesting
        assert!(db.add_in_subcategory("Bad", walk).is_err());
        assert!(db.add_in_subcategory("Bad", 999).is_err());
        assert!(db.add_in_subcategory("  ", buy).is_err());
        assert!(db.add_subcategory("", life).is_err());
        let other = db.add_subcategory("Other", life).unwrap();
        assert!(db.move_todo_to(other, life, Some(buy), 0).is_err());
        assert!(db.move_todo_to(walk, work, Some(buy), 0).is_err());
        assert!(db.move_todo_to(walk, life, Some(buy), 5).is_err());
        db.delete(other).unwrap();

        // Move a top-level todo into the sub-category, then reorder within it
        db.move_todo_to(walk, life, Some(buy), 1).unwrap();
        assert_eq!(titles(&db.list_subcategory_todos(buy)), ["Milk", "Walk", "Eggs"]);
        assert_eq!(titles(&db.list_category_todos(life)), ["Call mom", "To buy"]);
        // ...and back out to the top level
        db.move_todo(eggs, life, 0).unwrap();
        assert_eq!(titles(&db.list_category_todos(life)), ["Eggs", "Call mom", "To buy"]);
        assert_eq!(db.todo(eggs).unwrap().parent_id, None);

        // Moving a sub-category to another category carries its todos
        db.move_todo(buy, work, 0).unwrap();
        assert_eq!(titles(&db.list_category_todos(work)), ["To buy"]);
        assert_eq!(titles(&db.list_subcategory_todos(buy)), ["Milk", "Walk"]);
        assert!(db.list().iter().filter(|t| t.parent_id == Some(buy)).all(|t| t.category_id == work));

        let mut reopened = MemoryDb::open(&path).unwrap();
        assert_eq!(reopened.list(), db.list());
        reopened.close().unwrap();
        db.close().unwrap();
    }

    #[test]
    fn test_subcategory_completion_and_deletion_cascade() {
        let mut db = MemoryDb::open_in_memory().unwrap();
        let life = db.add_category("Life").unwrap();
        let buy = db.add_subcategory("To buy", life).unwrap();
        let milk = db.add_in_subcategory("Milk", buy).unwrap();
        let eggs = db.add_in_subcategory("Eggs", buy).unwrap();
        let keep = db.add_in_category("Keep", life).unwrap();

        db.set_done(buy, true).unwrap();
        assert!(db.todo(milk).unwrap().done && db.todo(eggs).unwrap().done);
        db.set_done(milk, false).unwrap();
        assert!(!db.todo(buy).unwrap().done);
        db.set_done(milk, true).unwrap();
        assert!(db.todo(buy).unwrap().done);
        db.set_done(buy, false).unwrap();
        assert!(!db.todo(milk).unwrap().done && !db.todo(eggs).unwrap().done);

        // Failed writes leave the cache untouched
        db.exec_sql_for_test(
            "CREATE TRIGGER block_done BEFORE UPDATE OF done ON todos WHEN NEW.id = 3
             BEGIN SELECT RAISE(ABORT, 'blocked'); END;",
        )
        .unwrap();
        let before = db.list();
        assert!(db.set_done(buy, true).is_err());
        assert_eq!(db.list(), before);
        db.exec_sql_for_test("DROP TRIGGER block_done").unwrap();

        db.delete(buy).unwrap();
        assert_eq!(titles(&db.list()), ["Keep"]);
        assert_eq!(db.list()[0].id, keep);
        let rows: i64 = db
            .conn()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM todos", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);

        let buy = db.add_subcategory("To buy", life).unwrap();
        db.add_in_subcategory("Bread", buy).unwrap();

        // Adding, moving, or deleting children keeps parent done state synchronized
        db.set_done(buy, true).unwrap();
        assert!(db.todo(buy).unwrap().done);
        let butter = db.add_in_subcategory("Butter", buy).unwrap();
        assert!(!db.todo(buy).unwrap().done);
        // Deleting the only unchecked child checks the parent again
        db.delete(butter).unwrap();
        assert!(db.todo(buy).unwrap().done);
        // Moving an unchecked top-level todo into a checked sub-category unchecks it
        let errand = db.add_in_category("Errand", life).unwrap();
        db.move_todo_to(errand, life, Some(buy), 1).unwrap();
        assert!(!db.todo(buy).unwrap().done);
        // Moving that only unchecked todo out re-checks the sub-category
        db.move_todo(errand, life, 0).unwrap();
        assert!(db.todo(buy).unwrap().done);
        // Deleting the last remaining child leaves the sub-category's done state unchanged
        let bread = db.list_subcategory_todos(buy)[0].id;
        db.delete(bread).unwrap();
        assert!(db.todo(buy).unwrap().done);
        // Empty title validation on a sub-category
        assert!(matches!(
            db.set_title(buy, "   "),
            Err(StoreError::Validation(msg)) if msg == "enter a sub-category name first"
        ));

        db.delete_category(life).unwrap();
        assert!(db.list().is_empty());
    }
}
