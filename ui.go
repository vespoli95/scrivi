package main

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

func todoUI(w fyne.Window, db *memoryDB) fyne.CanvasObject {
	entry := widget.NewEntry()
	entry.SetPlaceHolder("What needs doing?")
	rows := container.NewVBox()

	var refresh func()
	refresh = func() {
		rows.RemoveAll()
		items := db.list()
		for _, item := range items {
			check := widget.NewCheck(item.Title, nil)
			check.SetChecked(item.Done)
			check.OnChanged = func(done bool) {
				if err := db.setDone(item.ID, done); err != nil {
					refresh()
					dialog.ShowError(err, w)
					return
				}
				refresh()
			}
			remove := widget.NewButtonWithIcon("", theme.DeleteIcon(), func() {
				if err := db.delete(item.ID); err != nil {
					dialog.ShowError(err, w)
					return
				}
				refresh()
			})
			rows.Add(container.NewBorder(nil, nil, nil, remove, check))
		}
		if len(items) == 0 {
			rows.Add(widget.NewLabel("No todos yet. Add one above."))
		}
	}

	add := func() {
		if err := db.add(entry.Text); err != nil {
			dialog.ShowError(err, w)
			return
		}
		entry.SetText("")
		refresh()
		w.Canvas().Focus(entry)
	}
	entry.OnSubmitted = func(string) { add() }
	addButton := widget.NewButtonWithIcon("Add", theme.ContentAddIcon(), add)
	addButton.Importance = widget.HighImportance
	header := container.NewVBox(
		widget.NewLabelWithStyle("scrivi", fyne.TextAlignLeading, fyne.TextStyle{Bold: true, Italic: true}),
		container.NewBorder(nil, nil, nil, addButton, entry),
	)
	refresh()
	return container.NewBorder(header, nil, nil, nil, container.NewVScroll(rows))
}
