package main

import (
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestCategoriesPersistAndDeleteTheirTodos(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.close()
	for _, name := range []string{"  Life  ", "Money", "Work"} {
		if err := db.addCategory(name); err != nil {
			t.Fatal(err)
		}
	}
	groups := db.listCategories()
	if len(groups) != 3 || groups[0].Name != "Life" {
		t.Fatalf("unexpected categories: %+v", groups)
	}
	for i, group := range groups {
		if i > 0 && group.ID <= groups[i-1].ID {
			t.Fatalf("categories are not in insertion order: %+v", groups)
		}
		if err := db.addInCategory(group.Name+" task", group.ID); err != nil {
			t.Fatal(err)
		}
	}
	items := db.list()
	if err := db.setDone(items[1].ID, true); err != nil {
		t.Fatal(err)
	}
	reopened, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(reopened.listCategories(), groups) || !reflect.DeepEqual(reopened.list(), db.list()) {
		t.Fatal("category changes were not immediately durable")
	}
	if err := reopened.close(); err != nil {
		t.Fatal(err)
	}
	if err := db.deleteCategory(groups[0].ID); err != nil {
		t.Fatal(err)
	}
	if len(db.listCategories()) != 2 {
		t.Fatal("category was not removed")
	}
	items = db.list()
	if len(items) != 2 || items[0].Title != "Money task" || !items[0].Done {
		t.Fatalf("category deletion did not delete only its todos: %+v", items)
	}
	if items[0].CategoryID != groups[1].ID || items[1].CategoryID != groups[2].ID {
		t.Fatal("deletion changed another category's todos")
	}
	reopened, err = openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.close()
	if !reflect.DeepEqual(reopened.listCategories(), db.listCategories()) || !reflect.DeepEqual(reopened.list(), items) {
		t.Fatal("category deletion was not persisted")
	}
}

func TestInvalidCategoryOperations(t *testing.T) {
	db := newTestDB(t)
	if err := db.addCategory("Work"); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"", " \t\n ", " work "} {
		if err := db.addCategory(name); err == nil {
			t.Errorf("accepted invalid category %q", name)
		}
	}
	for _, id := range []int{0, 42} {
		if err := db.deleteCategory(id); !errors.Is(err, errCategoryNotFound) {
			t.Fatalf("unexpected category deletion error: %v", err)
		}
	}
	if err := db.addInCategory("Task", 42); !errors.Is(err, errCategoryNotFound) {
		t.Fatalf("unexpected category assignment error: %v", err)
	}
	if err := db.addInCategory("Task", 0); !errors.Is(err, errCategoryNotFound) {
		t.Fatalf("accepted uncategorized todo: %v", err)
	}
	if len(db.listCategories()) != 1 || len(db.list()) != 0 {
		t.Fatal("invalid operations changed stored data")
	}
}

func TestCategoryIconsPersistAndPreserveCacheOnFailure(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.close()
	if err := db.addCategoryWithIcon("Life", "🏠"); err != nil {
		t.Fatal(err)
	}
	id := db.listCategories()[0].ID
	checkPersisted := func(icon string) {
		t.Helper()
		reopened, err := openMemoryDB(path)
		if err != nil {
			t.Fatal(err)
		}
		defer reopened.close()
		if reopened.categories[id].Icon != icon || db.categories[id].Icon != icon {
			t.Fatalf("category icon was not durable: want %q", icon)
		}
	}
	checkPersisted("🏠")
	if err := db.setCategoryIcon(id, "⭐"); err != nil {
		t.Fatal(err)
	}
	checkPersisted("⭐")
	if err := db.setCategoryIcon(42, "⭐"); !errors.Is(err, errCategoryNotFound) {
		t.Fatalf("unexpected missing category error: %v", err)
	}
	if err := db.setCategoryIcon(id, "not an emoji"); err == nil {
		t.Fatal("accepted an invalid category icon")
	}
	if err := db.addCategoryWithIcon("Invalid", "not an emoji"); err == nil {
		t.Fatal("created a category with an invalid icon")
	}
	if _, err := db.disk.Exec(`CREATE TRIGGER prevent_icon_update BEFORE UPDATE OF icon ON categories
		BEGIN SELECT RAISE(ABORT, 'update blocked'); END`); err != nil {
		t.Fatal(err)
	}
	if err := db.setCategoryIcon(id, "💼"); err == nil {
		t.Fatal("expected category icon write error")
	}
	checkPersisted("⭐")
	if _, err := db.disk.Exec("DROP TRIGGER prevent_icon_update"); err != nil {
		t.Fatal(err)
	}
	if err := db.setCategoryIcon(id, ""); err != nil {
		t.Fatal(err)
	}
	checkPersisted("")
}

func TestOrderMovesAndThemesPersist(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.close()
	for _, name := range []string{"Life", "Work", "Home"} {
		if err := db.addCategory(name); err != nil {
			t.Fatal(err)
		}
	}
	groups := db.listCategories()
	for _, title := range []string{"A", "B", "C"} {
		if err := db.addInCategory(title, groups[0].ID); err != nil {
			t.Fatal(err)
		}
	}
	if err := db.addInCategory("D", groups[1].ID); err != nil {
		t.Fatal(err)
	}
	items := db.list()
	if err := db.setDone(items[1].ID, true); err != nil {
		t.Fatal(err)
	}
	if err := db.moveCategory(groups[2].ID, 0); err != nil {
		t.Fatal(err)
	}
	if err := db.moveTodo(items[2].ID, groups[0].ID, 0); err != nil {
		t.Fatal(err)
	}
	if err := db.moveTodo(items[1].ID, groups[1].ID, 0); err != nil {
		t.Fatal(err)
	}
	if err := db.setAppTheme("Ocean"); err != nil {
		t.Fatal(err)
	}
	if err := db.setCategoryColor(groups[1].ID, "Rose"); err != nil {
		t.Fatal(err)
	}
	if got := db.listCategories(); got[0].Name != "Home" || got[1].Name != "Life" || got[2].Name != "Work" {
		t.Fatalf("unexpected category order: %+v", got)
	}
	life, work := db.listCategoryTodos(groups[0].ID), db.listCategoryTodos(groups[1].ID)
	if len(life) != 2 || life[0].Title != "C" || life[1].Title != "A" || len(work) != 2 || work[0].Title != "B" || !work[0].Done || work[1].Title != "D" {
		t.Fatalf("unexpected todo order: life=%+v work=%+v", life, work)
	}
	reopened, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.close()
	if !reflect.DeepEqual(db.listCategories(), reopened.listCategories()) ||
		!reflect.DeepEqual(life, reopened.listCategoryTodos(groups[0].ID)) ||
		!reflect.DeepEqual(work, reopened.listCategoryTodos(groups[1].ID)) ||
		reopened.appTheme != "Ocean" || reopened.categories[groups[1].ID].Color != "Rose" {
		t.Fatal("moves or themes were not persisted")
	}
	if err := reopened.addInCategory("E", groups[1].ID); err != nil {
		t.Fatal(err)
	}
	if got := reopened.listCategoryTodos(groups[1].ID); got[len(got)-1].Title != "E" {
		t.Fatal("new todos should follow moved todos")
	}
}

func TestOrderAndThemeFailuresPreserveState(t *testing.T) {
	db := newTestDB(t)
	for _, name := range []string{"Life", "Work"} {
		if err := db.addCategory(name); err != nil {
			t.Fatal(err)
		}
	}
	groups := db.listCategories()
	for _, title := range []string{"A", "B"} {
		if err := db.addInCategory(title, groups[0].ID); err != nil {
			t.Fatal(err)
		}
	}
	items := db.list()
	if err := db.moveTodo(items[0].ID, 42, 0); !errors.Is(err, errCategoryNotFound) {
		t.Fatalf("unexpected missing target error: %v", err)
	}
	if err := db.moveTodo(items[0].ID, groups[1].ID, 2); err == nil {
		t.Fatal("accepted invalid todo position")
	}
	if err := db.moveCategory(groups[0].ID, -1); err == nil {
		t.Fatal("accepted invalid category position")
	}
	if err := db.setAppTheme("unknown"); err == nil {
		t.Fatal("accepted invalid theme")
	}
	if err := db.setCategoryColor(groups[0].ID, "unknown"); err == nil {
		t.Fatal("accepted invalid category color")
	}
	if _, err := db.disk.Exec(`CREATE TRIGGER prevent_reorder BEFORE UPDATE OF position ON todos
		WHEN OLD.id = 1 BEGIN SELECT RAISE(ABORT, 'reorder blocked'); END;
		CREATE TRIGGER prevent_category_reorder BEFORE UPDATE OF position ON categories
		WHEN OLD.id = 1 BEGIN SELECT RAISE(ABORT, 'reorder blocked'); END`); err != nil {
		t.Fatal(err)
	}
	if err := db.moveTodo(items[1].ID, groups[0].ID, 0); err == nil {
		t.Fatal("expected todo reorder failure")
	}
	if err := db.moveCategory(groups[1].ID, 0); err == nil {
		t.Fatal("expected category reorder failure")
	}
	if !reflect.DeepEqual(db.list(), items) || !reflect.DeepEqual(db.listCategories(), groups) || db.appTheme != defaultTheme {
		t.Fatal("failed operations changed cached state")
	}
	var position int
	if err := db.disk.QueryRow("SELECT position FROM todos WHERE id = ?", items[1].ID).Scan(&position); err != nil || position != items[1].Position {
		t.Fatalf("failed todo reorder changed disk: position=%d err=%v", position, err)
	}
}

func TestExistingCategoriesMigrateWithoutLosingData(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	legacy, err := sql.Open("sqlite3", path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(`PRAGMA foreign_keys = ON;
		CREATE TABLE categories (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL COLLATE NOCASE UNIQUE);
		CREATE TABLE todos (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT NOT NULL,
			done INTEGER NOT NULL DEFAULT 0, category_id INTEGER REFERENCES categories(id));
		INSERT INTO categories (id, name) VALUES (1, 'Work');
		INSERT INTO todos (title, done, category_id) VALUES ('Keep', 1, 1)`); err != nil {
		legacy.Close()
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.close()
	if db.categories[1] != (category{ID: 1, Name: "Work", Position: 1}) ||
		!reflect.DeepEqual(db.list(), []todo{{ID: 1, Title: "Keep", Done: true, CategoryID: 1, Position: 1}}) {
		t.Fatal("icon migration changed existing categories or todos")
	}
	if err := db.setCategoryIcon(1, "💼"); err != nil {
		t.Fatal(err)
	}
}

func TestCategoryDeletionRollsBackOnFailure(t *testing.T) {
	db := newTestDB(t)
	if err := db.addCategory("Work"); err != nil {
		t.Fatal(err)
	}
	id := db.listCategories()[0].ID
	if err := db.addInCategory("Keep", id); err != nil {
		t.Fatal(err)
	}
	expectedTodos, expectedCategories := db.list(), db.listCategories()
	if _, err := db.disk.Exec(`CREATE TRIGGER prevent_category_delete BEFORE DELETE ON categories
		BEGIN SELECT RAISE(ABORT, 'deletion blocked'); END`); err != nil {
		t.Fatal(err)
	}
	if err := db.deleteCategory(id); err == nil {
		t.Fatal("expected category deletion error")
	}
	if !reflect.DeepEqual(db.list(), expectedTodos) || !reflect.DeepEqual(db.listCategories(), expectedCategories) {
		t.Fatal("failed category deletion changed memory")
	}
	var savedCategory int
	if err := db.disk.QueryRow("SELECT category_id FROM todos WHERE id = ?", expectedTodos[0].ID).Scan(&savedCategory); err != nil {
		t.Fatal(err)
	}
	if savedCategory != id {
		t.Fatal("failed category deletion changed todos on disk")
	}
	if err := db.close(); err != nil {
		t.Fatal(err)
	}
	if err := db.addCategory("Unsaved"); err == nil {
		t.Fatal("expected category save error")
	}
	if err := db.addInCategory("Unsaved", id); err == nil {
		t.Fatal("expected categorized todo save error")
	}
	if !reflect.DeepEqual(db.list(), expectedTodos) || !reflect.DeepEqual(db.listCategories(), expectedCategories) {
		t.Fatal("failed writes changed memory")
	}
}

func TestExistingTodosMigrateToRecoveryCategory(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	legacy, err := sql.Open("sqlite3", path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := legacy.Exec(`CREATE TABLE todos (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		title TEXT NOT NULL,
		done INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1))
	);
	INSERT INTO todos (title, done) VALUES ('Keep', 0), ('Complete', 1)`); err != nil {
		legacy.Close()
		t.Fatal(err)
	}
	if err := legacy.Close(); err != nil {
		t.Fatal(err)
	}
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.close()
	groups := db.listCategories()
	if len(groups) != 1 || groups[0].Name != "Recovered todos" {
		t.Fatalf("missing recovery category: %+v", groups)
	}
	expected := []todo{{ID: 1, Title: "Keep", CategoryID: groups[0].ID, Position: 1}, {ID: 2, Title: "Complete", Done: true, CategoryID: groups[0].ID, Position: 2}}
	if !reflect.DeepEqual(db.list(), expected) {
		t.Fatalf("migration changed existing todos: %+v", db.list())
	}
	if err := db.addCategory("Work"); err != nil {
		t.Fatal(err)
	}
	if err := db.addInCategory("New", db.listCategories()[1].ID); err != nil {
		t.Fatal(err)
	}
	reopened, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.close()
	if !reflect.DeepEqual(reopened.list(), db.list()) {
		t.Fatal("reopening migrated database changed todos")
	}
}

func TestRecoveryAvoidsExistingCategoryNames(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	if err := db.addCategory("Recovered todos"); err != nil {
		t.Fatal(err)
	}
	if _, err := db.disk.Exec("INSERT INTO todos (title) VALUES ('Legacy')"); err != nil {
		t.Fatal(err)
	}
	if err := db.close(); err != nil {
		t.Fatal(err)
	}
	db, err = openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.close()
	groups := db.listCategories()
	if len(groups) != 2 || groups[1].Name != "Recovered todos 2" || db.list()[0].CategoryID != groups[1].ID {
		t.Fatalf("recovery merged or lost data: categories=%+v todos=%+v", groups, db.list())
	}
}

func TestRecoveryRollsBackOnFailure(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	if _, err := db.disk.Exec(`INSERT INTO todos (title) VALUES ('Legacy');
		CREATE TRIGGER prevent_recovery BEFORE UPDATE OF category_id ON todos
		BEGIN SELECT RAISE(ABORT, 'recovery blocked'); END`); err != nil {
		t.Fatal(err)
	}
	if err := db.close(); err != nil {
		t.Fatal(err)
	}
	if db, err := openMemoryDB(path); err == nil {
		db.close()
		t.Fatal("expected recovery to fail")
	}
	disk, err := sql.Open("sqlite3", path)
	if err != nil {
		t.Fatal(err)
	}
	defer disk.Close()
	var categories, unassigned int
	if err := disk.QueryRow("SELECT COUNT(*) FROM categories").Scan(&categories); err != nil {
		t.Fatal(err)
	}
	if err := disk.QueryRow("SELECT COUNT(*) FROM todos WHERE category_id IS NULL").Scan(&unassigned); err != nil {
		t.Fatal(err)
	}
	if categories != 0 || unassigned != 1 {
		t.Fatal("failed recovery did not roll back")
	}
}

func TestEditTodoPersistsAndPreservesStateOnFailure(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer db.close()
	if err := addTestTodo(db, "Original"); err != nil {
		t.Fatal(err)
	}
	id := db.list()[0].ID
	if err := db.setDone(id, true); err != nil {
		t.Fatal(err)
	}
	if err := db.setTitle(id, "  Edited  "); err != nil {
		t.Fatal(err)
	}
	expected := db.list()
	if expected[0].Title != "Edited" || !expected[0].Done {
		t.Fatalf("edit changed the wrong fields: %+v", expected[0])
	}
	reopened, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.close()
	if !reflect.DeepEqual(reopened.list(), expected) {
		t.Fatal("edited title was not persisted")
	}
	if err := db.setTitle(id, " \t "); err == nil {
		t.Fatal("accepted a blank title")
	}
	if err := db.setTitle(42, "Missing"); !errors.Is(err, errTodoNotFound) {
		t.Fatalf("unexpected missing todo error: %v", err)
	}
	if _, err := db.disk.Exec(`CREATE TRIGGER prevent_title_update BEFORE UPDATE OF title ON todos
		BEGIN SELECT RAISE(ABORT, 'update blocked'); END`); err != nil {
		t.Fatal(err)
	}
	if err := db.setTitle(id, "Unsaved"); err == nil {
		t.Fatal("expected a title write error")
	}
	var savedTitle string
	if err := reopened.disk.QueryRow("SELECT title FROM todos WHERE id = ?", id).Scan(&savedTitle); err != nil {
		t.Fatal(err)
	}
	if !reflect.DeepEqual(db.list(), expected) || savedTitle != expected[0].Title {
		t.Fatal("failed edit changed memory or disk")
	}
}

func addTestTodo(db *memoryDB, title string) error {
	groups := db.listCategories()
	if len(groups) == 0 {
		if err := db.addCategory("Test"); err != nil {
			return err
		}
		groups = db.listCategories()
	}
	return db.addInCategory(title, groups[0].ID)
}

func newTestDB(t *testing.T) *memoryDB {
	t.Helper()
	db, err := openMemoryDB(":memory:")
	if err != nil {
		t.Fatal(err)
	}
	t.Cleanup(func() {
		if err := db.close(); err != nil {
			t.Error(err)
		}
	})
	return db
}

func TestTodoLifecycle(t *testing.T) {
	db := newTestDB(t)
	if err := addTestTodo(db, "  Buy milk  "); err != nil {
		t.Fatal(err)
	}
	if err := addTestTodo(db, "Read a book"); err != nil {
		t.Fatal(err)
	}
	items := db.list()
	if len(items) != 2 || items[0].Title != "Buy milk" || items[0].Done {
		t.Fatalf("unexpected todos: %+v", items)
	}
	if items[0].ID >= items[1].ID {
		t.Fatalf("todos are not in insertion order: %+v", items)
	}
	id := items[0].ID
	if err := db.setDone(id, true); err != nil {
		t.Fatal(err)
	}
	if !db.list()[0].Done {
		t.Fatal("todo was not completed")
	}
	if err := db.setDone(id, false); err != nil {
		t.Fatal(err)
	}
	if db.list()[0].Done {
		t.Fatal("todo was not reopened")
	}
	if err := db.delete(id); err != nil {
		t.Fatal(err)
	}
	if remaining := db.list(); len(remaining) != 1 || remaining[0].Title != "Read a book" {
		t.Fatalf("unexpected todos after deletion: %+v", remaining)
	}
	if err := addTestTodo(db, "Third todo"); err != nil {
		t.Fatal(err)
	}
	if db.list()[1].ID <= items[1].ID {
		t.Fatal("a deleted ID was reused")
	}
	if len(newTestDB(t).list()) != 0 {
		t.Fatal("new databases must start empty")
	}
}

func TestInvalidOperations(t *testing.T) {
	db := newTestDB(t)
	for _, title := range []string{"", " \t\n "} {
		if err := addTestTodo(db, title); err == nil {
			t.Errorf("accepted blank title %q", title)
		}
	}
	if len(db.list()) != 0 {
		t.Fatal("invalid additions changed the database")
	}
	if err := db.setDone(42, true); !errors.Is(err, errTodoNotFound) {
		t.Fatalf("unexpected update error: %v", err)
	}
	if err := db.delete(42); !errors.Is(err, errTodoNotFound) {
		t.Fatalf("unexpected deletion error: %v", err)
	}
}

func TestListReturnsCopies(t *testing.T) {
	db := newTestDB(t)
	if err := addTestTodo(db, "Original"); err != nil {
		t.Fatal(err)
	}
	items := db.list()
	items[0].Title = "Changed"
	if db.list()[0].Title != "Original" {
		t.Fatal("changing a query result mutated the database")
	}
}

func TestPersistence(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	db, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer func() {
		if err := db.close(); err != nil {
			t.Error(err)
		}
	}()
	for _, title := range []string{"Keep", "Complete", "Delete"} {
		if err := addTestTodo(db, title); err != nil {
			t.Fatal(err)
		}
	}
	items := db.list()
	if err := db.setDone(items[1].ID, true); err != nil {
		t.Fatal(err)
	}
	if err := db.delete(items[2].ID); err != nil {
		t.Fatal(err)
	}
	expected := db.list()

	// Changes must be durable before closing, not just saved at shutdown.
	reopened, err := openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	if actual := reopened.list(); !reflect.DeepEqual(actual, expected) {
		t.Fatalf("loaded %+v, want %+v", actual, expected)
	}
	if err := reopened.close(); err != nil {
		t.Fatal(err)
	}
	if err := db.close(); err != nil {
		t.Fatal(err)
	}
	reopened, err = openMemoryDB(path)
	if err != nil {
		t.Fatal(err)
	}
	defer reopened.close()
	if actual := reopened.list(); !reflect.DeepEqual(actual, expected) {
		t.Fatalf("after restart loaded %+v, want %+v", actual, expected)
	}
	if err := addTestTodo(reopened, "New"); err != nil {
		t.Fatal(err)
	}
	if latest := reopened.list()[2]; latest.ID <= items[2].ID {
		t.Fatal("a deleted ID was reused after restart")
	}
}

func TestFailedWritesPreserveMemory(t *testing.T) {
	db := newTestDB(t)
	if err := addTestTodo(db, "Keep"); err != nil {
		t.Fatal(err)
	}
	expected := db.list()
	if err := db.close(); err != nil {
		t.Fatal(err)
	}
	for name, operation := range map[string]func() error{
		"add":      func() error { return addTestTodo(db, "Unsaved") },
		"complete": func() error { return db.setDone(expected[0].ID, true) },
		"delete":   func() error { return db.delete(expected[0].ID) },
	} {
		t.Run(name, func(t *testing.T) {
			if err := operation(); err == nil {
				t.Fatal("expected a storage error")
			}
			if actual := db.list(); !reflect.DeepEqual(actual, expected) {
				t.Fatalf("failed save changed memory: %+v", actual)
			}
		})
	}
}

func TestInvalidDatabaseFailsToOpen(t *testing.T) {
	path := filepath.Join(t.TempDir(), "todos.sqlite")
	if err := os.WriteFile(path, []byte("not a SQLite database"), 0600); err != nil {
		t.Fatal(err)
	}
	if db, err := openMemoryDB(path); err == nil {
		db.close()
		t.Fatal("corrupt database silently opened")
	}
}
