package main

import (
	"database/sql"
	"errors"
	"fmt"
	"sort"
	"strings"

	_ "github.com/mattn/go-sqlite3"
)

var (
	errTodoNotFound     = errors.New("todo not found")
	errCategoryNotFound = errors.New("category not found")
)

type todo struct {
	ID         int
	Title      string
	Done       bool
	CategoryID int
}

type category struct {
	ID   int
	Name string
	Icon string
}

// memoryDB belongs to the UI goroutine. SQLite writes succeed before the cache changes.
type memoryDB struct {
	disk       *sql.DB
	todos      map[int]todo
	categories map[int]category
}

func openMemoryDB(path string) (*memoryDB, error) {
	disk, err := sql.Open("sqlite3", path)
	if err != nil {
		return nil, fmt.Errorf("open todo database: %w", err)
	}
	disk.SetMaxOpenConns(1)
	db := &memoryDB{disk: disk, todos: make(map[int]todo), categories: make(map[int]category)}
	if err := db.load(); err != nil {
		return nil, errors.Join(err, disk.Close())
	}
	return db, nil
}

func (db *memoryDB) load() error {
	if _, err := db.disk.Exec(`PRAGMA foreign_keys = ON;
		CREATE TABLE IF NOT EXISTS categories (
			id INTEGER PRIMARY KEY AUTOINCREMENT,
			name TEXT NOT NULL COLLATE NOCASE UNIQUE,
			icon TEXT NOT NULL DEFAULT ''
		)`); err != nil {
		return fmt.Errorf("initialize category database: %w", err)
	}
	if _, err := db.disk.Exec(`CREATE TABLE IF NOT EXISTS todos (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		title TEXT NOT NULL,
		done INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1)),
		category_id INTEGER REFERENCES categories(id)
	)`); err != nil {
		return fmt.Errorf("initialize todo database: %w", err)
	}
	if err := db.migrateCategories(); err != nil {
		return err
	}
	if err := db.migrateCategoryIcons(); err != nil {
		return err
	}
	if err := db.recoverUncategorized(); err != nil {
		return err
	}
	if err := db.loadCategories(); err != nil {
		return err
	}
	rows, err := db.disk.Query("SELECT id, title, done, COALESCE(category_id, 0) FROM todos")
	if err != nil {
		return fmt.Errorf("load todos: %w", err)
	}
	defer rows.Close()
	for rows.Next() {
		var item todo
		if err := rows.Scan(&item.ID, &item.Title, &item.Done, &item.CategoryID); err != nil {
			return fmt.Errorf("read todo: %w", err)
		}
		db.todos[item.ID] = item
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("load todos: %w", err)
	}
	return nil
}

func (db *memoryDB) migrateCategories() error {
	rows, err := db.disk.Query("PRAGMA table_info(todos)")
	if err != nil {
		return fmt.Errorf("inspect todo schema: %w", err)
	}
	hasCategory := false
	for rows.Next() {
		var id, notNull, primaryKey int
		var name, columnType string
		var defaultValue sql.NullString
		if err := rows.Scan(&id, &name, &columnType, &notNull, &defaultValue, &primaryKey); err != nil {
			return errors.Join(fmt.Errorf("read todo schema: %w", err), rows.Close())
		}
		hasCategory = hasCategory || name == "category_id"
	}
	if err := errors.Join(rows.Err(), rows.Close()); err != nil {
		return fmt.Errorf("inspect todo schema: %w", err)
	}
	if !hasCategory {
		if _, err := db.disk.Exec("ALTER TABLE todos ADD COLUMN category_id INTEGER REFERENCES categories(id)"); err != nil {
			return fmt.Errorf("migrate todo categories: %w", err)
		}
	}
	return nil
}

func (db *memoryDB) loadCategories() error {
	rows, err := db.disk.Query("SELECT id, name, icon FROM categories")
	if err != nil {
		return fmt.Errorf("load categories: %w", err)
	}
	defer rows.Close()
	for rows.Next() {
		var item category
		if err := rows.Scan(&item.ID, &item.Name, &item.Icon); err != nil {
			return fmt.Errorf("read category: %w", err)
		}
		db.categories[item.ID] = item
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("load categories: %w", err)
	}
	return nil
}

func (db *memoryDB) close() error {
	return db.disk.Close()
}

func (db *memoryDB) addInCategory(title string, categoryID int) error {
	title = strings.TrimSpace(title)
	if title == "" {
		return errors.New("enter a todo first")
	}
	if _, ok := db.categories[categoryID]; !ok {
		return errCategoryNotFound
	}
	result, err := db.disk.Exec("INSERT INTO todos (title, category_id) VALUES (?, ?)", title, categoryID)
	if err != nil {
		return fmt.Errorf("save todo: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return fmt.Errorf("get saved todo ID: %w", err)
	}
	db.todos[int(id)] = todo{ID: int(id), Title: title, CategoryID: categoryID}
	return nil
}

func (db *memoryDB) addCategory(name string) error {
	return db.addCategoryWithIcon(name, "")
}

func (db *memoryDB) addCategoryWithIcon(name, icon string) error {
	if !validCategoryIcon(icon) {
		return errors.New("choose a category emoji from the picker")
	}
	name = strings.TrimSpace(name)
	if name == "" {
		return errors.New("enter a category name first")
	}
	for _, item := range db.listCategories() {
		if strings.EqualFold(item.Name, name) {
			return errors.New("a category with that name already exists")
		}
	}
	result, err := db.disk.Exec("INSERT INTO categories (name, icon) VALUES (?, ?)", name, icon)
	if err != nil {
		return fmt.Errorf("save category: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return fmt.Errorf("get saved category ID: %w", err)
	}
	db.categories[int(id)] = category{ID: int(id), Name: name, Icon: icon}
	return nil
}

func (db *memoryDB) migrateCategoryIcons() error {
	rows, err := db.disk.Query("PRAGMA table_info(categories)")
	if err != nil {
		return fmt.Errorf("inspect category schema: %w", err)
	}
	hasIcon := false
	for rows.Next() {
		var id, notNull, primaryKey int
		var name, columnType string
		var defaultValue sql.NullString
		if err := rows.Scan(&id, &name, &columnType, &notNull, &defaultValue, &primaryKey); err != nil {
			return errors.Join(fmt.Errorf("read category schema: %w", err), rows.Close())
		}
		hasIcon = hasIcon || name == "icon"
	}
	if err := errors.Join(rows.Err(), rows.Close()); err != nil {
		return fmt.Errorf("inspect category schema: %w", err)
	}
	if !hasIcon {
		if _, err := db.disk.Exec("ALTER TABLE categories ADD COLUMN icon TEXT NOT NULL DEFAULT ''"); err != nil {
			return fmt.Errorf("migrate category icons: %w", err)
		}
	}
	return nil
}

func (db *memoryDB) recoverUncategorized() error {
	var count int
	if err := db.disk.QueryRow("SELECT COUNT(*) FROM todos WHERE category_id IS NULL").Scan(&count); err != nil {
		return fmt.Errorf("check uncategorized todos: %w", err)
	}
	if count == 0 {
		return nil
	}
	tx, err := db.disk.Begin()
	if err != nil {
		return fmt.Errorf("begin todo recovery: %w", err)
	}
	name := "Recovered todos"
	for suffix := 2; ; suffix++ {
		var exists bool
		if err := tx.QueryRow("SELECT EXISTS(SELECT 1 FROM categories WHERE name = ? COLLATE NOCASE)", name).Scan(&exists); err != nil {
			return errors.Join(fmt.Errorf("find recovery category: %w", err), tx.Rollback())
		}
		if !exists {
			break
		}
		name = fmt.Sprintf("Recovered todos %d", suffix)
	}
	result, err := tx.Exec("INSERT INTO categories (name) VALUES (?)", name)
	if err != nil {
		return errors.Join(fmt.Errorf("create recovery category: %w", err), tx.Rollback())
	}
	id, err := result.LastInsertId()
	if err != nil {
		return errors.Join(fmt.Errorf("get recovery category ID: %w", err), tx.Rollback())
	}
	if _, err := tx.Exec("UPDATE todos SET category_id = ? WHERE category_id IS NULL", id); err != nil {
		return errors.Join(fmt.Errorf("recover uncategorized todos: %w", err), tx.Rollback())
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("save recovered todos: %w", err)
	}
	return nil
}

func (db *memoryDB) setCategoryIcon(id int, icon string) error {
	item, ok := db.categories[id]
	if !ok {
		return errCategoryNotFound
	}

	if !validCategoryIcon(icon) {
		return errors.New("choose a category emoji from the picker")
	}
	if _, err := db.disk.Exec("UPDATE categories SET icon = ? WHERE id = ?", icon, id); err != nil {
		return fmt.Errorf("save category icon: %w", err)
	}
	item.Icon = icon
	db.categories[id] = item
	return nil
}

var categoryEmojis = []string{
	"🏠", "💼", "💰", "📚", "🛒", "🎯",
	"❤️", "💪", "🍽️", "✈️", "🎨", "🎵",
	"🌱", "🐾", "💻", "🔧", "📅", "⭐",
	"🎮", "🚗", "🏖️", "🎁", "☕", "📝",
}

func validCategoryIcon(icon string) bool {
	if icon == "" {
		return true
	}
	for _, emoji := range categoryEmojis {
		if icon == emoji {
			return true
		}
	}
	return false
}

func (db *memoryDB) listCategories() []category {
	items := make([]category, 0, len(db.categories))
	for _, item := range db.categories {
		items = append(items, item)
	}
	sort.Slice(items, func(i, j int) bool {
		return items[i].ID < items[j].ID
	})
	return items
}

func (db *memoryDB) deleteCategory(id int) error {
	if _, ok := db.categories[id]; !ok {
		return errCategoryNotFound
	}
	tx, err := db.disk.Begin()
	if err != nil {
		return fmt.Errorf("begin category deletion: %w", err)
	}
	if _, err := tx.Exec("DELETE FROM todos WHERE category_id = ?", id); err != nil {
		return errors.Join(fmt.Errorf("delete category todos: %w", err), tx.Rollback())
	}
	if _, err := tx.Exec("DELETE FROM categories WHERE id = ?", id); err != nil {
		return errors.Join(fmt.Errorf("delete category: %w", err), tx.Rollback())
	}
	if err := tx.Commit(); err != nil {
		return fmt.Errorf("save category deletion: %w", err)
	}
	delete(db.categories, id)
	for todoID, item := range db.todos {
		if item.CategoryID == id {
			delete(db.todos, todoID)
		}
	}
	return nil
}

func (db *memoryDB) setTitle(id int, title string) error {
	item, ok := db.todos[id]
	if !ok {
		return errTodoNotFound
	}
	title = strings.TrimSpace(title)
	if title == "" {
		return errors.New("enter a todo first")
	}
	if _, err := db.disk.Exec("UPDATE todos SET title = ? WHERE id = ?", title, id); err != nil {
		return fmt.Errorf("save todo title: %w", err)
	}
	item.Title = title
	db.todos[id] = item
	return nil
}

func (db *memoryDB) list() []todo {
	todos := make([]todo, 0, len(db.todos))
	for _, item := range db.todos {
		todos = append(todos, item)
	}
	sort.Slice(todos, func(i, j int) bool {
		return todos[i].ID < todos[j].ID
	})
	return todos
}

func (db *memoryDB) setDone(id int, done bool) error {
	item, ok := db.todos[id]
	if !ok {
		return errTodoNotFound
	}
	if _, err := db.disk.Exec("UPDATE todos SET done = ? WHERE id = ?", done, id); err != nil {
		return fmt.Errorf("save completion: %w", err)
	}
	item.Done = done
	db.todos[id] = item
	return nil
}

func (db *memoryDB) delete(id int) error {
	if _, ok := db.todos[id]; !ok {
		return errTodoNotFound
	}
	if _, err := db.disk.Exec("DELETE FROM todos WHERE id = ?", id); err != nil {
		return fmt.Errorf("delete todo: %w", err)
	}
	delete(db.todos, id)
	return nil
}
