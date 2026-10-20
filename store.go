package main

import (
	"database/sql"
	"errors"
	"fmt"
	"sort"
	"strings"

	_ "github.com/mattn/go-sqlite3"
)

var errTodoNotFound = errors.New("todo not found")

type todo struct {
	ID    int
	Title string
	Done  bool
}

// memoryDB belongs to the UI goroutine. SQLite writes succeed before the cache changes.
type memoryDB struct {
	disk  *sql.DB
	todos map[int]todo
}

func openMemoryDB(path string) (*memoryDB, error) {
	disk, err := sql.Open("sqlite3", path)
	if err != nil {
		return nil, fmt.Errorf("open todo database: %w", err)
	}
	disk.SetMaxOpenConns(1)
	db := &memoryDB{disk: disk, todos: make(map[int]todo)}
	if err := db.load(); err != nil {
		return nil, errors.Join(err, disk.Close())
	}
	return db, nil
}

func (db *memoryDB) load() error {
	if _, err := db.disk.Exec(`CREATE TABLE IF NOT EXISTS todos (
		id INTEGER PRIMARY KEY AUTOINCREMENT,
		title TEXT NOT NULL,
		done INTEGER NOT NULL DEFAULT 0 CHECK (done IN (0, 1))
	)`); err != nil {
		return fmt.Errorf("initialize todo database: %w", err)
	}
	rows, err := db.disk.Query("SELECT id, title, done FROM todos")
	if err != nil {
		return fmt.Errorf("load todos: %w", err)
	}
	defer rows.Close()
	for rows.Next() {
		var item todo
		if err := rows.Scan(&item.ID, &item.Title, &item.Done); err != nil {
			return fmt.Errorf("read todo: %w", err)
		}
		db.todos[item.ID] = item
	}
	if err := rows.Err(); err != nil {
		return fmt.Errorf("load todos: %w", err)
	}
	return nil
}

func (db *memoryDB) close() error {
	return db.disk.Close()
}

func (db *memoryDB) add(title string) error {
	title = strings.TrimSpace(title)
	if title == "" {
		return errors.New("enter a todo first")
	}
	result, err := db.disk.Exec("INSERT INTO todos (title) VALUES (?)", title)
	if err != nil {
		return fmt.Errorf("save todo: %w", err)
	}
	id, err := result.LastInsertId()
	if err != nil {
		return fmt.Errorf("get saved todo ID: %w", err)
	}
	db.todos[int(id)] = todo{ID: int(id), Title: title}
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
