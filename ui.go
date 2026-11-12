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

type pointerButton struct {
	widget.Button
}

type categoryEmojiButton struct {
	widget.BaseWidget
	emoji      string
	selected   bool
	onTapped   func()
	label      *canvas.Text
	background *canvas.Rectangle
	content    *fyne.Container
	size       fyne.Size
}

func newCategoryEmojiButton(emoji string, selected bool, size fyne.Size, tapped func()) *categoryEmojiButton {
	display := emoji
	if display == "" {
		display = "+"
	}
	background := canvas.NewRectangle(theme.ButtonColor())
	background.CornerRadius = theme.InputRadiusSize()
	label := canvas.NewText(display, theme.ForegroundColor())
	label.TextSize = 28
	label.Alignment = fyne.TextAlignCenter
	button := &categoryEmojiButton{
		emoji: emoji, selected: selected, onTapped: tapped, label: label,
		background: background, content: container.NewStack(background, container.NewCenter(label)), size: size,
	}
	button.ExtendBaseWidget(button)
	button.updateStyle()
	return button
}

func (b *categoryEmojiButton) CreateRenderer() fyne.WidgetRenderer {
	return widget.NewSimpleRenderer(b.content)
}

func (b *categoryEmojiButton) MinSize() fyne.Size { return b.size }

func (b *categoryEmojiButton) SetEmoji(emoji string) {
	b.emoji = emoji
	if emoji == "" {
		b.label.Text = "+"
	} else {
		b.label.Text = emoji
	}
	b.Refresh()
}

func (b *categoryEmojiButton) updateStyle() {
	if b.selected {
		b.background.FillColor = theme.PrimaryColor()
		b.label.Color = theme.Color(theme.ColorNameForegroundOnPrimary)
	} else {
		b.background.FillColor = theme.ButtonColor()
		b.label.Color = theme.ForegroundColor()
	}
	b.background.Refresh()
	b.label.Refresh()
}

func (b *categoryEmojiButton) Refresh() {
	b.updateStyle()
	b.BaseWidget.Refresh()
}

func (b *categoryEmojiButton) Tapped(*fyne.PointEvent) { b.onTapped() }
func (b *categoryEmojiButton) Cursor() desktop.Cursor  { return desktop.PointerCursor }

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
		value := emoji
		button := newCategoryEmojiButton(emoji, emoji == selected, fyne.NewSize(48, 48), func() {
			d.Hide()
			onSelected(value)
		})
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

func showCategoryColorPicker(w fyne.Window, selected string, onSelected func(string)) {
	choices := container.NewGridWithColumns(2)
	d := dialog.NewCustomWithoutButtons("Category color", choices, w)
	for _, name := range categoryColors {
		value := name
		if name == "Default" {
			value = ""
		}
		button := newPointerButton(name, nil, func() {
			d.Hide()
			onSelected(value)
		})
		if value == selected {
			button.Importance = widget.HighImportance
		}
		choices.Add(button)
	}
	d.SetButtons([]fyne.CanvasObject{newPointerButton("Cancel", theme.CancelIcon(), d.Hide)})
	d.Show()
}

func showTodoEditor(w fyne.Window, db *memoryDB, item todo, refresh func()) {
	entry := widget.NewEntry()
	entry.SetText(item.Title)
	d := dialog.NewCustomWithoutButtons("Edit todo", entry, w)
	d.Resize(fyne.NewSize(600, 320))
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

type categoryHeadingLayout struct {
	icon  fyne.CanvasObject
	label *widget.Label
}

func (l *categoryHeadingLayout) MinSize([]fyne.CanvasObject) fyne.Size {
	return l.icon.MinSize().AddWidthHeight(theme.Padding()+l.label.MinSize().Width, 0).Max(l.label.MinSize())
}

func (l *categoryHeadingLayout) Layout(_ []fyne.CanvasObject, size fyne.Size) {
	iconSize := l.icon.MinSize()
	labelWidth := fyne.MeasureText(l.label.Text, theme.TextSize(), l.label.TextStyle).Width + theme.InnerPadding()*4
	labelWidth = min(labelWidth, max(0, size.Width-iconSize.Width-theme.Padding()))
	width := iconSize.Width + theme.Padding() + labelWidth
	start := max(0, (size.Width-width)/2)
	l.icon.Move(fyne.NewPos(start, (size.Height-iconSize.Height)/2))
	l.icon.Resize(iconSize)
	l.label.Move(fyne.NewPos(start+iconSize.Width+theme.Padding(), 0))
	l.label.Resize(fyne.NewSize(labelWidth, size.Height))
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
	id       int
	content  fyne.CanvasObject
	surface  *categorySurface
	scroll   *nestedScroll
	itemRows []*todoRow
	refresh  func()
}

type categorySurface struct {
	widget.BaseWidget
	db          *memoryDB
	color       string
	highlighted bool
}

func newCategorySurface(db *memoryDB) *categorySurface {
	surface := &categorySurface{db: db}
	surface.ExtendBaseWidget(surface)
	return surface
}

func (s *categorySurface) CreateRenderer() fyne.WidgetRenderer {
	background := canvas.NewRectangle(categoryColor(s.color, s.db.appTheme))
	background.CornerRadius = theme.InputRadiusSize() * 2
	return &categorySurfaceRenderer{WidgetRenderer: widget.NewSimpleRenderer(background), surface: s, background: background}
}

type categorySurfaceRenderer struct {
	fyne.WidgetRenderer
	surface    *categorySurface
	background *canvas.Rectangle
}

func (r *categorySurfaceRenderer) Refresh() {
	r.background.FillColor = categoryColor(r.surface.color, r.surface.db.appTheme)
	r.background.CornerRadius = theme.InputRadiusSize() * 2
	r.background.StrokeWidth = 0
	if r.surface.highlighted {
		r.background.StrokeWidth = 2
		r.background.StrokeColor = theme.Color(theme.ColorNamePrimary)
	}
	r.WidgetRenderer.Refresh()
}

func newTodoCategoryPanel(w fyne.Window, db *memoryDB, group category, onDelete func(), drag *dragController) *todoCategoryPanel {
	entry := widget.NewEntry()
	entry.SetPlaceHolder("What needs doing?")
	rowsLayout := &todoRowsLayout{width: 340}
	rows := container.New(rowsLayout)
	scroll := newNestedScroll(rows, rowsLayout, drag.scroll)
	panel := &todoCategoryPanel{id: group.ID, surface: newCategorySurface(db), scroll: scroll}

	var refresh func()
	refresh = func() {
		offset := scroll.Offset
		rowsLayout.rows = nil
		panel.itemRows = nil
		rows.RemoveAll()
		items := db.listCategoryTodos(group.ID)
		for _, item := range items {
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
			}).(*todoTitleWidget)
			handle := newDragHandle(drag.hover, func(point fyne.Position) { drag.dropTodo(item.ID, point) })
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
			left := container.NewHBox(handle, check)
			right := container.NewHBox(edit, remove)
			row := &todoRow{id: item.ID, title: title, left: left, right: right}
			row.content = container.NewBorder(nil, nil, left, right, title)
			rowsLayout.rows = append(rowsLayout.rows, row)
			panel.itemRows = append(panel.itemRows, row)
			rows.Add(row.content)
		}
		if len(items) == 0 {
			empty := widget.NewLabel("No todos yet. Add one above.")
			empty.Importance = widget.LowImportance
			rows.Add(empty)
		}
		scroll.Refresh()
		scroll.ScrollToOffset(offset)
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
	addButton := newPointerButton("", theme.ContentAddIcon(), add)
	addButton.Importance = widget.HighImportance
	name := widget.NewLabelWithStyle(group.Name, fyne.TextAlignCenter, fyne.TextStyle{Bold: true})
	deleteButton := newPointerButton("", theme.DeleteIcon(), onDelete)
	deleteButton.Importance = widget.LowImportance
	iconButton := newCategoryEmojiButton(group.Icon, false, fyne.NewSize(40, 40), func() {
		showCategoryEmojiPicker(w, db.categories[group.ID].Icon, func(icon string) {
			if err := db.setCategoryIcon(group.ID, icon); err != nil {
				showError(err, w)
				return
			}
			refresh()
		})
	})
	colorButton := newPointerButton("", theme.ColorPaletteIcon(), func() {
		showCategoryColorPicker(w, db.categories[group.ID].Color, func(value string) {
			if err := db.setCategoryColor(group.ID, value); err != nil {
				showError(err, w)
				return
			}
			refresh()
		})
	})
	colorButton.Importance = widget.LowImportance
	handle := newDragHandle(drag.hover, func(point fyne.Position) { drag.dropCategory(group.ID, point) })
	name.Truncation = fyne.TextTruncateEllipsis
	refreshRows := refresh
	refresh = func() {
		group = db.categories[group.ID]
		name.SetText(group.Name)
		iconButton.SetEmoji(group.Icon)
		panel.surface.color = group.Color
		panel.surface.Refresh()
		refreshRows()
	}
	controls := container.NewHBox(colorButton, deleteButton)
	controlSize := controls.MinSize()
	titleBar := container.NewBorder(nil, nil,
		container.NewGridWrap(controlSize, container.NewHBox(handle)),
		controls, container.New(&categoryHeadingLayout{icon: iconButton, label: name}, iconButton, name))
	header := container.NewVBox(
		titleBar,
		widget.NewSeparator(),
		container.NewBorder(nil, nil, nil, addButton, entry),
	)
	refresh()
	card := widget.NewCard("", "", container.NewBorder(header, nil, nil, nil, scroll))
	panel.content = container.NewStack(panel.surface, card)
	panel.refresh = refresh
	return panel
}

func todoUI(w fyne.Window, db *memoryDB) fyne.CanvasObject {
	entry := widget.NewEntry()
	entry.SetPlaceHolder("Category name (Life, Money, Work...)")
	panels := container.New(&categoryGridLayout{cellSize: fyne.NewSize(340, 240), columns: 2})
	categoryPanels := make(map[int]*todoCategoryPanel)
	scroll := container.NewVScroll(panels)
	drag := &dragController{window: w, db: db, scroll: scroll, panels: categoryPanels}

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
				}, drag)
				categoryPanels[group.ID] = panel
			}
			panel.refresh()
			panels.Add(panel.content)
		}
	}
	drag.refresh = refresh
	var selectedIcon string
	var iconButton *categoryEmojiButton
	add := func() {
		if err := db.addCategoryWithIcon(entry.Text, selectedIcon); err != nil {
			showError(err, w)
			return
		}
		entry.SetText("")
		selectedIcon = ""
		iconButton.SetEmoji("")
		refresh()
		w.Canvas().Focus(entry)
	}
	entry.OnSubmitted = func(string) { add() }
	iconButton = newCategoryEmojiButton("", false, fyne.NewSize(40, 40), func() {
		showCategoryEmojiPicker(w, selectedIcon, func(icon string) {
			selectedIcon = icon
			iconButton.SetEmoji(icon)
		})
	})
	addButton := newPointerButton("Add category", theme.ContentAddIcon(), add)
	addButton.Importance = widget.HighImportance
	title := widget.NewLabelWithStyle("scrivi", fyne.TextAlignLeading, fyne.TextStyle{Bold: true, Italic: true})
	header := container.NewVBox(
		container.NewBorder(nil, nil, nil, windowOptions(w, db), title),
		container.NewBorder(nil, nil, iconButton, addButton, entry),
	)
	refresh()
	return container.NewPadded(container.NewBorder(header, nil, nil, nil, scroll))
}
