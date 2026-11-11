//go:build !windows

package main

import "fyne.io/fyne/v2"

func windowOptions(w fyne.Window, db *memoryDB) fyne.CanvasObject {
	return newOptionsButton(w, db, nil, nil)
}
