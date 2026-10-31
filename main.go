package main

import (
	_ "embed"
	"fmt"
	"log"
	"os"
	"path/filepath"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/app"
)

//go:embed assets/icon.svg
var appIconData []byte

func main() {
	a := app.New()
	icon := fyne.NewStaticResource("scrivi.svg", appIconData)
	a.SetIcon(icon)
	w := a.NewWindow("Scrivi")
	w.SetIcon(icon)
	db, err := openDefaultDB()
	if err != nil {
		log.Print(err)
		message := newErrorDialog(err, w)
		message.SetOnClosed(w.Close)
		message.Show()
		w.ShowAndRun()
		return
	}
	defer func() {
		if err := db.close(); err != nil {
			log.Printf("close todo database: %v", err)
		}
	}()
	w.SetContent(todoUI(w, db))
	w.Resize(fyne.NewSize(760, 600))
	checkForUpdates(a, w)
	w.ShowAndRun()
}

func openDefaultDB() (*memoryDB, error) {
	root, err := os.UserConfigDir()
	if err != nil {
		return nil, fmt.Errorf("find application data directory: %w", err)
	}
	dir := filepath.Join(root, "scrivi")
	if err := os.MkdirAll(dir, 0700); err != nil {
		return nil, fmt.Errorf("create application data directory: %w", err)
	}
	return openMemoryDB(filepath.Join(dir, "todos.sqlite"))
}
