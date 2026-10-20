package main

import (
	"errors"
	"os"
	"path/filepath"
	"reflect"
	"testing"
)

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
