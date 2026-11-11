package main

import (
	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/dialog"
	"fyne.io/fyne/v2/driver/desktop"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

type todoTitleLayout struct {
	title string
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

func (t *todoTitleWidget) Cursor() desktop.Cursor {
	return desktop.PointerCursor
}

func (l *todoTitleLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	return fyne.MeasureText("…", theme.TextSize(), fyne.TextStyle{}).
		Add(fyne.NewSquareSize(theme.InnerPadding() * 2))
}

func (l *todoTitleLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	text := objects[0].(*canvas.Text)
	padding := theme.InnerPadding()
	text.TextSize = theme.TextSize()
	text.Text = truncateTodoTitle(l.title, max(0, size.Width-padding*2), text.TextSize)
	text.Color = theme.ForegroundColor()
	if len(objects) > 1 {
		text.Color = theme.DisabledColor()
	}
	textSize := text.MinSize()
	text.Move(fyne.NewPos(padding, (size.Height-textSize.Height)/2))
	text.Resize(textSize)
	text.Refresh()
	if len(objects) == 1 {
		return
	}

	line := objects[1].(*canvas.Line)
	line.StrokeColor = text.Color
	line.Move(fyne.NewPos(text.Position().X, size.Height/2))
	line.Resize(fyne.NewSize(textSize.Width, 0))
	if text.Text == "" {
		line.Hide()
	} else {
		line.Show()
	}
	line.Refresh()
}

func truncateTodoTitle(title string, width, textSize float32) string {
	measure := func(text string) float32 {
		return fyne.MeasureText(text, textSize, fyne.TextStyle{}).Width
	}
	if measure(title) <= width {
		return title
	}
	if measure("…") > width {
		return ""
	}
	runes := []rune(title)
	low, high := 0, len(runes)
	for low < high {
		mid := (low + high + 1) / 2
		if measure(string(runes[:mid])+"…") <= width {
			low = mid
		} else {
			high = mid - 1
		}
	}
	return string(runes[:low]) + "…"
}

func newTodoTitle(title string, done bool, onTapped func()) fyne.CanvasObject {
	text := canvas.NewText(title, theme.ForegroundColor())
	objects := []fyne.CanvasObject{text}
	if done {
		objects = append(objects, canvas.NewLine(theme.DisabledColor()))
	}
	return newTodoTitleWidget(container.New(&todoTitleLayout{title: title}, objects...), onTapped)
}

type pointerButton struct {
	widget.Button
}

func newPointerButton(text string, icon fyne.Resource, tapped func()) *pointerButton {
	button := &pointerButton{Button: widget.Button{Text: text, Icon: icon, OnTapped: tapped}}
	button.ExtendBaseWidget(button)
	return button
}

func (b *pointerButton) Cursor() desktop.Cursor {
	if b.Disabled() {
		return desktop.DefaultCursor
	}
	return desktop.PointerCursor
}

type pointerCheck struct {
	widget.Check
}

func newPointerCheck() *pointerCheck {
	check := &pointerCheck{}
	check.ExtendBaseWidget(check)
	return check
}

func (c *pointerCheck) Cursor() desktop.Cursor {
	if c.Disabled() {
		return desktop.DefaultCursor
	}
	return desktop.PointerCursor
}

func newErrorDialog(err error, w fyne.Window) dialog.Dialog {
	message := widget.NewLabel(err.Error())
	message.Wrapping = fyne.TextWrapWord
	d := dialog.NewCustomWithoutButtons("Error", message, w)
	d.SetIcon(theme.ErrorIcon())
	d.SetButtons([]fyne.CanvasObject{newPointerButton("OK", theme.ConfirmIcon(), d.Hide)})
	return d
}

func showError(err error, w fyne.Window) {
	newErrorDialog(err, w).Show()
}

func showConfirm(title, message string, callback func(bool), w fyne.Window) {
	label := widget.NewLabel(message)
	label.Wrapping = fyne.TextWrapWord
	d := dialog.NewCustomWithoutButtons(title, label, w)
	confirmed := false
	d.SetOnClosed(func() { callback(confirmed) })
	yes := newPointerButton("Yes", theme.ConfirmIcon(), func() {
		confirmed = true
		d.Hide()
	})
	yes.Importance = widget.HighImportance
	d.SetButtons([]fyne.CanvasObject{
		newPointerButton("No", theme.CancelIcon(), d.Hide), yes,
	})
	d.Show()
}

func showCategoryEmojiPicker(w fyne.Window, selected string, onSelected func(string)) {
	choices := container.NewGridWithColumns(6)
	d := dialog.NewCustomWithoutButtons("Category emoji", choices, w)
	for _, emoji := range categoryEmojis {
		button := newPointerButton(emoji, nil, func() {
			d.Hide()
			onSelected(emoji)
		})
		if emoji == selected {
			button.Importance = widget.HighImportance
		}
		choices.Add(button)
	}
	d.SetButtons([]fyne.CanvasObject{
		newPointerButton("No icon", nil, func() {
			d.Hide()
			onSelected("")
		}),
		newPointerButton("Cancel", theme.CancelIcon(), d.Hide),
	})
	d.Show()
}

func showTodoEditor(w fyne.Window, db *memoryDB, item todo, refresh func()) {
	entry := widget.NewEntry()
	entry.SetText(item.Title)
	d := dialog.NewCustomWithoutButtons("Edit todo", entry, w)
	save := func() {
		if err := db.setTitle(item.ID, entry.Text); err != nil {
			showError(err, w)
			return
		}
		d.Hide()
		refresh()
	}
	entry.OnSubmitted = func(string) { save() }
	saveButton := newPointerButton("Save", theme.ConfirmIcon(), save)
	saveButton.Importance = widget.HighImportance
	d.SetButtons([]fyne.CanvasObject{
		newPointerButton("Cancel", theme.CancelIcon(), d.Hide), saveButton,
	})
	d.Show()
	w.Canvas().Focus(entry)
}

type categoryGridLayout struct {
	cellSize fyne.Size
	rows     int
	columns  int
}

func (l *categoryGridLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	padding := theme.Padding() * 3
	columns := max(1, int((size.Width+padding)/(l.cellSize.Width+padding)))
	l.columns = columns
	var visible []fyne.CanvasObject
	for _, object := range objects {
		if object.Visible() {
			visible = append(visible, object)
		}
	}
	l.rows = (len(visible) + columns - 1) / columns
	cellHeight := l.cellSize.Height
	if l.rows > 0 {
		cellHeight = max(cellHeight, (size.Height-float32(l.rows-1)*padding)/float32(l.rows))
	}
	for start := 0; start < len(visible); start += columns {
		count := min(columns, len(visible)-start)
		width := max(0, (size.Width-float32(count-1)*padding)/float32(count))
		for column := 0; column < count; column++ {
			object := visible[start+column]
			object.Move(fyne.NewPos(float32(column)*(width+padding),
				float32(start/columns)*(cellHeight+padding)))
			object.Resize(fyne.NewSize(width, cellHeight))
		}
	}
}

func (l *categoryGridLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	columns := max(1, l.columns)
	count := 0
	for _, object := range objects {
		if object.Visible() {
			count++
		}
	}
	rows := max(1, (count+columns-1)/columns)
	l.rows = rows
	return fyne.NewSize(l.cellSize.Width,
		float32(rows)*l.cellSize.Height+float32(rows-1)*theme.Padding()*3)
}

type todoCategoryPanel struct {
	content fyne.CanvasObject
	refresh func()
}

type categorySurface struct {
	widget.BaseWidget
}

func newCategorySurface() *categorySurface {
	surface := &categorySurface{}
	surface.ExtendBaseWidget(surface)
	return surface
}

func (s *categorySurface) CreateRenderer() fyne.WidgetRenderer {
	background := canvas.NewRectangle(theme.ColorForWidget(theme.ColorNameHeaderBackground, s))
	background.CornerRadius = theme.InputRadiusSize() * 2
	return &categorySurfaceRenderer{WidgetRenderer: widget.NewSimpleRenderer(background), surface: s, background: background}
}

type categorySurfaceRenderer struct {
	fyne.WidgetRenderer
	surface    *categorySurface
	background *canvas.Rectangle
}

func (r *categorySurfaceRenderer) Refresh() {
	r.background.FillColor = theme.ColorForWidget(theme.ColorNameHeaderBackground, r.surface)
	r.background.CornerRadius = theme.InputRadiusSize() * 2
	r.WidgetRenderer.Refresh()
}

func newTodoCategoryPanel(w fyne.Window, db *memoryDB, group category, onDelete func()) *todoCategoryPanel {
	entry := widget.NewEntry()
	entry.SetPlaceHolder("What needs doing?")
	rows := container.NewVBox()

	var refresh func()
	refresh = func() {
		rows.RemoveAll()
		items := db.list()
		count := 0
		for _, item := range items {
			if item.CategoryID != group.ID {
				continue
			}
			count++
			check := newPointerCheck()
			check.SetChecked(item.Done)
			check.OnChanged = func(done bool) {
				if err := db.setDone(item.ID, done); err != nil {
					refresh()
					showError(err, w)
					return
				}
				refresh()
			}
			title := newTodoTitle(item.Title, item.Done, func() {
				check.SetChecked(!check.Checked)
			})
			edit := newPointerButton("", theme.DocumentCreateIcon(), func() {
				showTodoEditor(w, db, item, refresh)
			})
			edit.Importance = widget.LowImportance
			remove := newPointerButton("", theme.DeleteIcon(), func() {
				if err := db.delete(item.ID); err != nil {
					showError(err, w)
					return
				}
				refresh()
			})
			remove.Importance = widget.LowImportance
			rows.Add(container.NewBorder(nil, nil, check, container.NewHBox(edit, remove), title))
		}
		if count == 0 {
			empty := widget.NewLabel("No todos yet. Add one above.")
			empty.Importance = widget.LowImportance
			rows.Add(empty)
		}
	}

	add := func() {
		if err := db.addInCategory(entry.Text, group.ID); err != nil {
			showError(err, w)
			return
		}
		entry.SetText("")
		refresh()
		w.Canvas().Focus(entry)
	}
	entry.OnSubmitted = func(string) { add() }
	addButton := newPointerButton("Add", theme.ContentAddIcon(), add)
	addButton.Importance = widget.HighImportance
	name := widget.NewLabelWithStyle(group.Name, fyne.TextAlignCenter, fyne.TextStyle{Bold: true})
	deleteButton := newPointerButton("", theme.DeleteIcon(), onDelete)
	deleteButton.Importance = widget.LowImportance
	iconButton := newPointerButton("Icon", nil, func() {
		showCategoryEmojiPicker(w, db.categories[group.ID].Icon, func(icon string) {
			if err := db.setCategoryIcon(group.ID, icon); err != nil {
				showError(err, w)
				return
			}
			refresh()
		})
	})
	iconButton.Importance = widget.LowImportance
	name.Truncation = fyne.TextTruncateEllipsis
	refreshRows := refresh
	refresh = func() {
		group = db.categories[group.ID]
		title := group.Name
		if group.Icon != "" {
			title = group.Icon + " " + title
		}
		name.SetText(title)
		refreshRows()
	}
	// Equal-width title-bar controls keep the category name centered.
	controlSize := iconButton.MinSize().Max(deleteButton.MinSize())
	titleBar := container.NewBorder(nil, nil,
		container.NewGridWrap(controlSize, iconButton),
		container.NewGridWrap(controlSize, deleteButton), name)
	header := container.NewVBox(
		titleBar,
		widget.NewSeparator(),
		container.NewBorder(nil, nil, nil, addButton, entry),
	)
	refresh()
	card := widget.NewCard("", "", container.NewBorder(header, nil, nil, nil, container.NewVScroll(rows)))
	return &todoCategoryPanel{
		content: container.NewStack(newCategorySurface(), card),
		refresh: refresh,
	}
}

func todoUI(w fyne.Window, db *memoryDB) fyne.CanvasObject {
	entry := widget.NewEntry()
	entry.SetPlaceHolder("Category name (Life, Money, Work...)")
	panels := container.New(&categoryGridLayout{cellSize: fyne.NewSize(340, 240), columns: 2})
	categoryPanels := make(map[int]*todoCategoryPanel)

	var refresh func()
	refresh = func() {
		panels.RemoveAll()
		for id := range categoryPanels {
			if _, exists := db.categories[id]; !exists {
				delete(categoryPanels, id)
			}
		}
		if len(db.categories) == 0 {
			panels.Add(container.NewCenter(widget.NewLabel("Create a category to get started.")))
		}
		for _, group := range db.listCategories() {
			panel, exists := categoryPanels[group.ID]
			if !exists {
				panel = newTodoCategoryPanel(w, db, group, func() {
					showConfirm("Delete category?",
						"Delete "+group.Name+" and all its todos? This cannot be undone.", func(confirmed bool) {
							if !confirmed {
								return
							}
							if err := db.deleteCategory(group.ID); err != nil {
								showError(err, w)
								return
							}
							refresh()
						}, w)
				})
				categoryPanels[group.ID] = panel
			}
			panel.refresh()
			panels.Add(panel.content)
		}
	}
	var selectedIcon string
	var iconButton *pointerButton
	add := func() {
		if err := db.addCategoryWithIcon(entry.Text, selectedIcon); err != nil {
			showError(err, w)
			return
		}
		entry.SetText("")
		selectedIcon = ""
		iconButton.SetText("Emoji")
		refresh()
		w.Canvas().Focus(entry)
	}
	entry.OnSubmitted = func(string) { add() }
	iconButton = newPointerButton("Emoji", nil, func() {
		showCategoryEmojiPicker(w, selectedIcon, func(icon string) {
			selectedIcon = icon
			if icon == "" {
				iconButton.SetText("Emoji")
			} else {
				iconButton.SetText(icon)
			}
		})
	})
	addButton := newPointerButton("Add category", theme.ContentAddIcon(), add)
	addButton.Importance = widget.HighImportance
	title := widget.NewLabelWithStyle("scrivi", fyne.TextAlignLeading, fyne.TextStyle{Bold: true, Italic: true})
	header := container.NewVBox(
		container.NewBorder(nil, nil, nil, windowOptions(w), title),
		container.NewBorder(nil, nil, iconButton, addButton, entry),
	)
	refresh()
	return container.NewPadded(container.NewBorder(header, nil, nil, nil, container.NewVScroll(panels)))
}
