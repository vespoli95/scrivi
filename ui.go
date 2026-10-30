package main

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

type todoTitleLayout struct {
	strikeWidth float32
}

type todoTitleWidget struct {
	widget.BaseWidget
	content  fyne.CanvasObject
	onTapped func()
}

func newTodoTitleWidget(content fyne.CanvasObject, onTapped func()) *todoTitleWidget {
	title := &todoTitleWidget{content: content, onTapped: onTapped}
	title.ExtendBaseWidget(title)
	return title
}

func (t *todoTitleWidget) CreateRenderer() fyne.WidgetRenderer {
	return widget.NewSimpleRenderer(t.content)
}

func (t *todoTitleWidget) Tapped(*fyne.PointEvent) {
	t.onTapped()
}

func (l *todoTitleLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	return objects[0].MinSize()
}

func (l *todoTitleLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	objects[0].Resize(size)
	if len(objects) == 1 {
		return
	}

	line := objects[1].(*canvas.Line)
	line.Move(fyne.NewPos(0, size.Height/2))
	line.Resize(fyne.NewSize(l.strikeWidth, 0))
}

func newTodoTitle(title string, done bool, onTapped func()) fyne.CanvasObject {
	label := widget.NewLabel(title)
	if !done {
		return newTodoTitleWidget(label, onTapped)
	}

	label.Importance = widget.LowImportance
	width := fyne.MeasureText(title, theme.TextSize(), fyne.TextStyle{}).Width
	line := canvas.NewLine(theme.DisabledColor())
	return newTodoTitleWidget(container.New(&todoTitleLayout{strikeWidth: width}, label, line), onTapped)
}

func todoUI(w fyne.Window, db *memoryDB) fyne.CanvasObject {
	entry := widget.NewEntry()
	entry.SetPlaceHolder("What needs doing?")
	rows := container.NewVBox()

	var refresh func()
	refresh = func() {
		rows.RemoveAll()
		items := db.list()
		for _, item := range items {
			check := widget.NewCheck("", nil)
			check.SetChecked(item.Done)
			check.OnChanged = func(done bool) {
				if err := db.setDone(item.ID, done); err != nil {
					refresh()
					dialog.ShowError(err, w)
					return
				}
				refresh()
			}
			title := newTodoTitle(item.Title, item.Done, func() {
				check.SetChecked(!check.Checked)
			})
			remove := widget.NewButtonWithIcon("", theme.DeleteIcon(), func() {
				if err := db.delete(item.ID); err != nil {
					dialog.ShowError(err, w)
					return
				}
				refresh()
			})
			rows.Add(container.NewBorder(nil, nil, nil, remove, container.NewHBox(check, title)))
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
