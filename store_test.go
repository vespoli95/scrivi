package main

import (
	"database/sql"
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

func TestCategoriesPersistAndDeleteWithoutLosingTodos(t *testing.T) {
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
	if len(groups) != 4 || groups[0].Name != "Uncategorized" || groups[1].Name != "Life" {
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
	if err := db.deleteCategory(groups[1].ID); err != nil {
		t.Fatal(err)
	}
	if len(db.listCategories()) != 3 {
		t.Fatal("category was not removed")
	}
	items = db.list()
	if len(items) != 4 || items[1].CategoryID != 0 || !items[1].Done || items[1].Title != "Life task" {
		t.Fatalf("category deletion lost todo data: %+v", items)
	}
	if items[2].CategoryID != groups[2].ID || items[3].CategoryID != groups[3].ID {
		t.Fatal("deletion moved another category's todos")
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
	for _, name := range []string{"", " \t\n ", " work ", "UNCATEGORIZED"} {
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
	if len(db.listCategories()) != 2 || len(db.list()) != 0 {
		t.Fatal("invalid operations changed stored data")
	}
}

func TestCategoryDeletionRollsBackOnFailure(t *testing.T) {
	db := newTestDB(t)
	if err := db.addCategory("Work"); err != nil {
		t.Fatal(err)
	}
	id := db.listCategories()[1].ID
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
		t.Fatal("failed category deletion moved todos on disk")
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

func TestExistingTodosMigrateToUncategorized(t *testing.T) {
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
	expected := []todo{{ID: 1, Title: "Keep"}, {ID: 2, Title: "Complete", Done: true}}
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
	if err := db.add("  Buy milk  "); err != nil {
		t.Fatal(err)
	}
	if err := db.add("Read a book"); err != nil {
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
	if err := db.add("Third todo"); err != nil {
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
		if err := db.add(title); err == nil {
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
	if err := db.add("Original"); err != nil {
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
		if err := db.add(title); err != nil {
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
	if err := reopened.add("New"); err != nil {
		t.Fatal(err)
	}
	if latest := reopened.list()[2]; latest.ID <= items[2].ID {
		t.Fatal("a deleted ID was reused after restart")
	}
}

func TestFailedWritesPreserveMemory(t *testing.T) {
	db := newTestDB(t)
	if err := db.add("Keep"); err != nil {
		t.Fatal(err)
	}
	expected := db.list()
	if err := db.close(); err != nil {
		t.Fatal(err)
	}
	for name, operation := range map[string]func() error{
		"add":      func() error { return db.add("Unsaved") },
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
