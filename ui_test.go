package main

import (
	"errors"
	"math"
	"strings"
	"testing"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/driver/desktop"
	"fyne.io/fyne/v2/test"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

func findUIObject[T fyne.CanvasObject](t *testing.T, root fyne.CanvasObject, matches func(T) bool) T {
	t.Helper()
	var found T
	var visit func(fyne.CanvasObject) bool
	visit = func(object fyne.CanvasObject) bool {
		if item, ok := object.(T); ok && matches(item) {
			found = item
			return true
		}
		switch item := object.(type) {
		case *fyne.Container:
			for _, child := range item.Objects {
				if visit(child) {
					return true
				}
			}
		case *container.Scroll:
			return visit(item.Content)
		case *nestedScroll:
			return visit(item.Content)
		case *widget.Card:
			return visit(item.Content)
		case *widget.PopUp:
			return visit(item.Content)
		case *todoTitleWidget:
			return visit(item.content)
		}
		return false
	}
	if !visit(root) {
		t.Fatalf("could not find matching %T", found)
	}
	return found
}

func TestCategoryUI(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	w := a.NewWindow("Scrivi")
	defer w.Close()
	db := newTestDB(t)
	content := todoUI(w, db)
	w.SetContent(content)
	w.Resize(fyne.NewSize(760, 600))
	w.Show()

	findButton := func(root fyne.CanvasObject, text string) *pointerButton {
		t.Helper()
		return findUIObject(t, root, func(button *pointerButton) bool { return button.Text == text })
	}
	findEmojiButton := func(root fyne.CanvasObject, emoji string) *categoryEmojiButton {
		t.Helper()
		return findUIObject(t, root, func(button *categoryEmojiButton) bool { return button.emoji == emoji })
	}
	findPanel := func(name string) *widget.Card {
		t.Helper()
		return findUIObject(t, content, func(card *widget.Card) bool {
			header := card.Content.(*fyne.Container).Objects[1].(*fyne.Container)
			var containsName func(fyne.CanvasObject) bool
			containsName = func(object fyne.CanvasObject) bool {
				if label, ok := object.(*widget.Label); ok {
					return label.Text == name || strings.HasSuffix(label.Text, " "+name)
				}
				if group, ok := object.(*fyne.Container); ok {
					for _, child := range group.Objects {
						if containsName(child) {
							return true
						}
					}
				}
				return false
			}
			return containsName(header.Objects[0])
		})
	}
	findTodoEntry := func(panel *widget.Card) *widget.Entry {
		t.Helper()
		return findUIObject(t, panel, func(entry *widget.Entry) bool { return true })
	}
	if len(db.listCategories()) != 0 {
		t.Fatal("new database has a default category")
	}
	findUIObject(t, content, func(label *widget.Label) bool { return label.Text == "Create a category to get started." })
	categoryEntry := findUIObject(t, content, func(entry *widget.Entry) bool {
		return entry.PlaceHolder == "Category name (Life, Money, Work...)"
	})
	categoryEntry.SetText("Life")
	test.Tap(findEmojiButton(content, ""))
	emojiTile := findEmojiButton(w.Canvas().Overlays().Top(), "🏠")
	if emojiTile.MinSize() != fyne.NewSize(48, 48) || len(categoryEmojis) <= 24 {
		t.Fatal("emoji picker choices are not a larger, expanded set")
	}
	test.Tap(emojiTile)
	test.Tap(findButton(content, "Add category"))
	if db.listCategories()[0].Icon != "🏠" || findEmojiButton(content, "") == nil {
		t.Fatal("category emoji was not saved and reset")
	}
	life := findPanel("Life")
	draft := findTodoEntry(life)
	draft.SetText("Keep this draft")
	categoryEntry.SetText("Work")
	categoryEntry.OnSubmitted(categoryEntry.Text)
	if len(db.listCategories()) != 2 || categoryEntry.Text != "" {
		t.Fatal("category controls did not add and clear the input")
	}
	if draft.Text != "Keep this draft" || findPanel("Life") != life {
		t.Fatal("adding a category discarded an existing panel's draft")
	}
	work := findPanel("Work")
	findUIObject(t, life, func(label *widget.Label) bool { return label.Text == "Life" })
	if life.Size().Width <= 340 || life.Size().Height <= 240 {
		t.Fatal("category panel does not stretch to fill the available window height")
	}
	findTodoEntry(life).SetText("Another draft")
	test.Tap(findEmojiButton(life, "🏠"))
	test.Tap(findEmojiButton(w.Canvas().Overlays().Top(), "⭐"))
	if db.listCategories()[0].Icon != "⭐" || findTodoEntry(life).Text != "Another draft" {
		t.Fatal("changing the category emoji lost its draft or did not save")
	}
	findUIObject(t, life, func(label *widget.Label) bool {
		return label.Text == "Life" && label.Alignment == fyne.TextAlignCenter
	})
	test.Tap(findEmojiButton(life, "⭐"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "Cancel"))
	if db.listCategories()[0].Icon != "⭐" {
		t.Fatal("canceling the emoji picker changed the category")
	}
	test.Tap(findEmojiButton(life, "⭐"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "No icon"))
	if db.listCategories()[0].Icon != "" {
		t.Fatal("category emoji could not be removed")
	}
	lifeHeader := life.Content.(*fyne.Container).Objects[1]
	addIcon := findEmojiButton(lifeHeader, "")
	test.Tap(addIcon)
	test.Tap(findEmojiButton(w.Canvas().Overlays().Top(), "🌱"))
	if db.listCategories()[0].Icon != "🌱" {
		t.Fatal("category plus did not add an icon")
	}
	colorButton := findUIObject(t, lifeHeader, func(button *pointerButton) bool {
		return button.Icon != nil && button.Icon.Name() == theme.ColorPaletteIcon().Name()
	})
	test.Tap(colorButton)
	test.Tap(findButton(w.Canvas().Overlays().Top(), "Rose"))
	if db.listCategories()[0].Color != "Rose" {
		t.Fatal("category color was not changed")
	}
	test.Tap(colorButton)
	test.Tap(findButton(w.Canvas().Overlays().Top(), "Default"))
	if db.listCategories()[0].Color != "" {
		t.Fatal("category color could not be reset")
	}
	entry := findTodoEntry(work)
	entry.SetText("Finish report")
	addTodo := findUIObject(t, work, func(button *pointerButton) bool {
		return button.Importance == widget.HighImportance && button.Icon != nil && button.Icon.Name() == theme.ContentAddIcon().Name()
	})
	test.Tap(addTodo)
	item := db.list()[0]
	if item.Title != "Finish report" || item.CategoryID != db.listCategories()[1].ID || entry.Text != "" {
		t.Fatalf("todo was not added to Work: %+v", item)
	}
	title := findUIObject(t, work, func(title *todoTitleWidget) bool { return true })
	test.Tap(title)
	if !db.list()[0].Done {
		t.Fatal("category todo could not be completed by tapping its title")
	}
	edit := findUIObject(t, work, func(button *pointerButton) bool {
		return button.Icon != nil && button.Icon.Name() == theme.DocumentCreateIcon().Name()
	})
	test.Tap(edit)
	editor := w.Canvas().Overlays().Top()
	editEntry := findUIObject(t, editor, func(entry *widget.Entry) bool { return true })
	if editEntry.Text != "Finish report" {
		t.Fatal("editor did not show existing title")
	}
	editEntry.SetText(" ")
	test.Tap(findButton(editor, "Save"))
	if db.list()[0].Title != "Finish report" {
		t.Fatal("blank title was saved")
	}
	test.Tap(findButton(w.Canvas().Overlays().Top(), "OK"))
	editEntry.SetText("Finish presentation")
	if _, err := db.disk.Exec(`CREATE TRIGGER prevent_edit BEFORE UPDATE OF title ON todos
		BEGIN SELECT RAISE(ABORT, 'update blocked'); END`); err != nil {
		t.Fatal(err)
	}
	test.Tap(findButton(editor, "Save"))
	if db.list()[0].Title != "Finish report" || editEntry.Text != "Finish presentation" {
		t.Fatal("failed edit changed the todo or lost the draft")
	}
	test.Tap(findButton(w.Canvas().Overlays().Top(), "OK"))
	if _, err := db.disk.Exec("DROP TRIGGER prevent_edit"); err != nil {
		t.Fatal(err)
	}
	editEntry.OnSubmitted(editEntry.Text)
	if db.list()[0].Title != "Finish presentation" || !db.list()[0].Done {
		t.Fatal("editing did not preserve completion")
	}
	edit = findUIObject(t, work, func(button *pointerButton) bool {
		return button.Icon != nil && button.Icon.Name() == theme.DocumentCreateIcon().Name()
	})
	test.Tap(edit)
	editor = w.Canvas().Overlays().Top()
	editEntry = findUIObject(t, editor, func(entry *widget.Entry) bool { return true })
	editEntry.SetText("Canceled edit")
	test.Tap(findButton(editor, "Cancel"))
	if db.list()[0].Title != "Finish presentation" {
		t.Fatal("canceling an edit changed the todo")
	}
	findTodoEntry(life).SetText("Another draft")
	workHeader := work.Content.(*fyne.Container).Objects[1]
	deleteCategory := findUIObject(t, workHeader, func(button *pointerButton) bool {
		return button.Icon != nil && button.Icon.Name() == theme.DeleteIcon().Name()
	})
	test.Tap(deleteCategory)
	findUIObject(t, w.Canvas().Overlays().Top(), func(label *widget.Label) bool {
		return strings.Contains(label.Text, "all its todos")
	})
	test.Tap(findButton(w.Canvas().Overlays().Top(), "No"))
	if len(db.listCategories()) != 2 || len(db.list()) != 1 {
		t.Fatal("canceling category deletion removed the category")
	}
	test.Tap(deleteCategory)
	test.Tap(findButton(w.Canvas().Overlays().Top(), "Yes"))
	if len(db.listCategories()) != 1 || len(db.list()) != 0 {
		t.Fatal("category deletion did not delete its todo")
	}
	if findTodoEntry(life).Text != "Another draft" {
		t.Fatal("deleting a category discarded another panel's draft")
	}
}

func TestWindowOptions(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	w := a.NewWindow("Scrivi")
	defer w.Close()
	pinned, startup := false, true
	failPin, failStartup, failRead := false, false, false
	pinButton := newPinButton(w, func(value bool) error {
		if failPin {
			return errors.New("pin failed")
		}
		pinned = value
		return nil
	})
	db := newTestDB(t)
	button := newOptionsButton(w, db, func() (bool, error) {
		if failRead {
			return false, errors.New("read failed")
		}
		return startup, nil
	}, func(value bool) error {
		if failStartup {
			return errors.New("startup failed")
		}
		startup = value
		return nil
	})
	w.SetContent(container.NewHBox(pinButton, button))
	w.Show()
	if pinButton.Icon == nil || pinButton.Text != "Pin" {
		t.Fatal("pin control is not visible on the main screen")
	}
	test.Tap(pinButton)
	if !pinned || pinButton.Text != "Unpin" || pinButton.Importance != widget.HighImportance {
		t.Fatal("pin control did not indicate the pinned state")
	}
	failPin = true
	test.Tap(pinButton)
	if !pinned || pinButton.Text != "Unpin" || pinButton.Importance != widget.HighImportance {
		t.Fatal("failed pin change altered the displayed state")
	}
	test.Tap(findUIObject(t, w.Canvas().Overlays().Top(), func(button *pointerButton) bool { return button.Text == "OK" }))
	failPin = false
	test.Tap(pinButton)
	if pinned || pinButton.Text != "Pin" || pinButton.Importance == widget.HighImportance {
		t.Fatal("unpin did not restore the unpinned state")
	}
	test.Tap(button)
	settings := w.Canvas().Overlays().Top()
	themeSelect := findUIObject(t, settings, func(selectBox *widget.Select) bool { return true })
	themeSelect.SetSelected("Ocean")
	if db.appTheme != "Ocean" {
		t.Fatal("theme selection was not applied")
	}
	if _, err := db.disk.Exec(`CREATE TRIGGER prevent_theme_update BEFORE UPDATE ON settings
		BEGIN SELECT RAISE(ABORT, 'theme blocked'); END`); err != nil {
		t.Fatal(err)
	}
	themeSelect.SetSelected("Rose")
	if themeSelect.Selected != "Ocean" || db.appTheme != "Ocean" {
		t.Fatal("failed theme change was not reverted")
	}
	test.Tap(findUIObject(t, w.Canvas().Overlays().Top(), func(button *pointerButton) bool { return button.Text == "OK" }))
	startupCheck := findUIObject(t, settings, func(check *pointerCheck) bool { return check.Text == "Start Scrivi when I sign in" })
	if !startupCheck.Checked {
		t.Fatal("settings do not reflect current state")
	}
	test.Tap(startupCheck)
	if startup {
		t.Fatal("settings changes were not applied")
	}
	failStartup = true
	test.Tap(startupCheck)
	if startupCheck.Checked || startup {
		t.Fatal("failed startup change was not reverted")
	}
	test.Tap(findUIObject(t, w.Canvas().Overlays().Top(), func(button *pointerButton) bool { return button.Text == "OK" }))
	failStartup = false
	test.Tap(startupCheck)
	if !startup {
		t.Fatal("settings did not recover after an error")
	}
	test.Tap(findUIObject(t, settings, func(button *pointerButton) bool { return button.Text == "Close" }))
	failRead = true
	test.Tap(button)
	findUIObject(t, w.Canvas().Overlays().Top(), func(label *widget.Label) bool { return label.Text == "read failed" })
	test.Tap(findUIObject(t, w.Canvas().Overlays().Top(), func(button *pointerButton) bool { return button.Text == "OK" }))
	settings = w.Canvas().Overlays().Top()
	startupCheck = findUIObject(t, settings, func(check *pointerCheck) bool { return check.Text == "Start Scrivi when I sign in" })
	if !startupCheck.Disabled() {
		t.Fatal("startup control should be disabled when its state cannot be read")
	}
	test.Tap(findUIObject(t, settings, func(button *pointerButton) bool { return button.Text == "Close" }))
	test.Tap(pinButton)
	if !pinned {
		t.Fatal("startup read failure prevented pinning")
	}
}

func TestDragCategoriesAndTodos(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	w := a.NewWindow("Scrivi")
	defer w.Close()
	db := newTestDB(t)
	for _, name := range []string{"Life", "Work", "Home"} {
		if err := db.addCategory(name); err != nil {
			t.Fatal(err)
		}
	}
	groups := db.listCategories()
	for _, title := range []string{"A", "B", "C"} {
		if err := db.addInCategory(title, groups[0].ID); err != nil {
			t.Fatal(err)
		}
	}
	if err := db.addInCategory("D", groups[1].ID); err != nil {
		t.Fatal(err)
	}
	content := todoUI(w, db)
	w.SetContent(content)
	w.Resize(fyne.NewSize(1200, 600))
	w.Show()
	grid := findUIObject(t, content, func(c *fyne.Container) bool {
		_, ok := c.Layout.(*categoryGridLayout)
		return ok
	})
	card := func(index int) *widget.Card {
		return grid.Objects[index].(*fyne.Container).Objects[1].(*widget.Card)
	}
	row := func(c *widget.Card, index int) *fyne.Container {
		scroll := findUIObject(t, c, func(s *nestedScroll) bool { return true })
		return scroll.Content.(*fyne.Container).Objects[index].(*fyne.Container)
	}
	point := func(object fyne.CanvasObject, x, y float32) fyne.Position {
		pos := a.Driver().AbsolutePositionForObject(object)
		return pos.Add(fyne.NewPos(object.Size().Width*x, object.Size().Height*y))
	}
	drop := func(handle *dragHandle, pos fyne.Position) {
		handle.Dragged(&fyne.DragEvent{PointEvent: fyne.PointEvent{AbsolutePosition: pos}})
		handle.DragEnd()
	}
	life, work := card(0), card(1)
	handle := findUIObject(t, row(life, 2), func(h *dragHandle) bool { return true })
	drop(handle, point(row(life, 0), 0.5, 0.25))
	if got := db.listCategoryTodos(groups[0].ID); got[0].Title != "C" || got[1].Title != "A" || got[2].Title != "B" {
		t.Fatalf("todo drag did not reorder: %+v", got)
	}
	life, work = card(0), card(1)
	handle = findUIObject(t, row(life, 2), func(h *dragHandle) bool { return true })
	drop(handle, point(row(work, 0), 0.5, 0.25))
	if got := db.listCategoryTodos(groups[1].ID); len(got) != 2 || got[0].Title != "B" || got[1].Title != "D" {
		t.Fatalf("todo drag did not move to category: %+v", got)
	}
	work = card(1)
	header := work.Content.(*fyne.Container).Objects[1]
	handle = findUIObject(t, header, func(h *dragHandle) bool { return true })
	drop(handle, point(card(0), 0.1, 0.5))
	if got := db.listCategories(); got[0].Name != "Work" || got[1].Name != "Life" {
		t.Fatalf("category drag did not reorder: %+v", got)
	}
	life, home := card(1), card(2)
	handle = findUIObject(t, row(life, 0), func(h *dragHandle) bool { return true })
	drop(handle, point(home, 0.5, 0.5))
	if got := db.listCategoryTodos(groups[2].ID); len(got) != 1 || got[0].Title != "C" {
		t.Fatalf("todo drag did not move to empty category: %+v", got)
	}
	w.Resize(fyne.NewSize(400, 900))
	work = card(0)
	header = work.Content.(*fyne.Container).Objects[1]
	handle = findUIObject(t, header, func(h *dragHandle) bool { return true })
	drop(handle, point(card(1), 0.2, 0.8))
	if got := db.listCategories(); got[0].Name != "Life" || got[1].Name != "Work" {
		t.Fatalf("vertical category drag did not use the target's lower half: %+v", got)
	}
}

func TestNestedScrollPropagatesAtEdges(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	w := a.NewWindow("Scrivi")
	defer w.Close()
	db := newTestDB(t)
	for _, name := range []string{"Life", "Work", "Home", "Other"} {
		if err := db.addCategory(name); err != nil {
			t.Fatal(err)
		}
	}
	content := todoUI(w, db)
	w.SetContent(content)
	w.Resize(fyne.NewSize(400, 400))
	w.Show()
	outer := findUIObject(t, content, func(s *container.Scroll) bool {
		_, ok := s.Content.(*fyne.Container).Layout.(*categoryGridLayout)
		return ok
	})
	inner := findUIObject(t, content, func(s *nestedScroll) bool { return true })
	pos := a.Driver().AbsolutePositionForObject(inner).Add(fyne.NewPos(20, 20))
	test.Scroll(w.Canvas(), pos, 0, -80)
	if outer.Offset.Y <= 0 {
		t.Fatal("scrolling over an empty category did not scroll the category grid")
	}
	offset := outer.Offset.Y
	entry := findUIObject(t, content, func(entry *widget.Entry) bool {
		return entry.PlaceHolder == "Category name (Life, Money, Work...)"
	})
	entry.SetText("Another")
	test.Tap(findUIObject(t, content, func(button *pointerButton) bool { return button.Text == "Add category" }))
	if outer.Offset.Y != offset {
		t.Fatalf("adding a category changed scroll position from %v to %v", offset, outer.Offset.Y)
	}
}

func TestDragAutoScrollTargets(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	w := a.NewWindow("Scrivi")
	defer w.Close()
	db := newTestDB(t)
	for _, name := range []string{"Life", "Work", "Home", "Other"} {
		if err := db.addCategory(name); err != nil {
			t.Fatal(err)
		}
	}
	groups := db.listCategories()
	for i := 0; i < 20; i++ {
		if err := db.addInCategory("Task", groups[0].ID); err != nil {
			t.Fatal(err)
		}
	}
	grid := container.New(&categoryGridLayout{cellSize: fyne.NewSize(340, 240), columns: 1})
	outer := container.NewVScroll(grid)
	drag := &dragController{window: w, db: db, scroll: outer, panels: map[int]*todoCategoryPanel{}}
	for _, group := range groups {
		panel := newTodoCategoryPanel(w, db, group, func() {}, drag)
		drag.panels[group.ID] = panel
		grid.Add(panel.content)
	}
	w.SetContent(outer)
	w.Resize(fyne.NewSize(400, 400))
	w.Show()
	outerPos := a.Driver().AbsolutePositionForObject(outer)
	drag.scrollNear(outerPos.Add(fyne.NewPos(20, outer.Size().Height-5)))
	if outer.Offset.Y <= 0 {
		t.Fatal("dragging at the grid edge did not scroll categories")
	}
	outer.ScrollToTop()
	inner := drag.panels[groups[0].ID].scroll
	innerPos := a.Driver().AbsolutePositionForObject(inner)
	drag.scrollNear(innerPos.Add(fyne.NewPos(20, inner.Size().Height-5)))
	if inner.Offset.Y <= 0 {
		t.Fatal("dragging at the category edge did not scroll todos")
	}
}

func TestWrappedTodosResizeAndScroll(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	w := a.NewWindow("Scrivi")
	defer w.Close()
	db := newTestDB(t)
	if err := db.addCategory("Life"); err != nil {
		t.Fatal(err)
	}
	id := db.listCategories()[0].ID
	for _, title := range []string{
		"A long task with several words that should wrap across multiple lines in a narrow category",
		"Another long task with several words to check that the rows do not overlap each other",
		"A third task that keeps the list taller than the available space",
		"And a fourth long task to make the inner list scroll",
	} {
		if err := db.addInCategory(title, id); err != nil {
			t.Fatal(err)
		}
	}
	if err := db.addCategory("Work"); err != nil {
		t.Fatal(err)
	}
	if err := db.addCategory("Home"); err != nil {
		t.Fatal(err)
	}
	content := todoUI(w, db)
	w.SetContent(content)
	w.Resize(fyne.NewSize(400, 360))
	w.Show()
	scroll := findUIObject(t, content, func(s *nestedScroll) bool { return true })
	verify := func() {
		t.Helper()
		rows := scroll.Content.(*fyne.Container)
		var bottom float32
		for _, object := range rows.Objects {
			row := object.(*fyne.Container)
			title := findUIObject(t, row, func(t *todoTitleWidget) bool { return true })
			if title.Size().Height+0.01 < title.heightFor(title.Size().Width) {
				t.Fatalf("wrapped title is clipped: size=%+v required=%v", title.Size(), title.heightFor(title.Size().Width))
			}
			if row.Position().Y+0.01 < bottom {
				t.Fatal("wrapped rows overlap")
			}
			bottom = row.Position().Y + row.Size().Height
		}
		if bottom > rows.Size().Height+0.01 {
			t.Fatalf("scroll content excludes wrapped rows: bottom=%v height=%v", bottom, rows.Size().Height)
		}
	}
	verify()
	pos := a.Driver().AbsolutePositionForObject(scroll).Add(fyne.NewPos(20, 20))
	test.Scroll(w.Canvas(), pos, 0, -80)
	if scroll.Offset.Y <= 0 {
		t.Fatal("long category did not scroll its own items")
	}
	outer := findUIObject(t, content, func(s *container.Scroll) bool {
		group, ok := s.Content.(*fyne.Container)
		if !ok {
			return false
		}
		_, ok = group.Layout.(*categoryGridLayout)
		return ok
	})
	scroll.ScrollToBottom()
	test.Scroll(w.Canvas(), pos, 0, -80)
	if outer.Offset.Y <= 0 {
		t.Fatal("scrolling at the end of a category did not continue scrolling the grid")
	}
	outer.ScrollToTop()
	w.Resize(fyne.NewSize(760, 360))
	verify()
	w.Resize(fyne.NewSize(400, 360))
	verify()
}

func TestTodoStrikethroughFollowsRenderedText(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	for _, title := range []string{"Buy milk", "A very long completed todo that wraps", "Résumé 日本語"} {
		for _, width := range []float32{200, 80} {
			widget := newTodoTitle(title, true, func() {}).(*todoTitleWidget)
			widget.Resize(fyne.NewSize(width, widget.heightFor(width)))
			var rendered []string
			for i := 0; i < len(widget.content.Objects); i += 2 {
				text := widget.content.Objects[i].(*canvas.Text)
				line := widget.content.Objects[i+1].(*canvas.Line)
				rendered = append(rendered, text.Text)
				if line.Position1.X != text.Position().X || math.Abs(float64(line.Position2.X-text.Position().X-text.MinSize().Width)) > 0.01 {
					t.Fatalf("strike does not follow rendered text %q", text.Text)
				}
			}
			if strings.ReplaceAll(strings.Join(rendered, ""), " ", "") != strings.ReplaceAll(title, " ", "") {
				t.Fatalf("wrapped text lost content: %q", rendered)
			}
		}
	}
}

func TestCategoryRowsFillAvailableWidth(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	for _, width := range []float32{300, 340, 760, 1200} {
		objects := []fyne.CanvasObject{
			canvas.NewRectangle(nil), canvas.NewRectangle(nil), canvas.NewRectangle(nil),
			canvas.NewRectangle(nil), canvas.NewRectangle(nil),
		}
		layout := &categoryGridLayout{cellSize: fyne.NewSize(340, 320)}
		layout.Layout(objects, fyne.NewSize(width, 600))
		for start := 0; start < len(objects); {
			end := start
			for end+1 < len(objects) && objects[end+1].Position().Y == objects[start].Position().Y {
				end++
			}
			if objects[start].Position().X != 0 || math.Abs(float64(width-objects[end].Position().X-objects[end].Size().Width)) > 0.01 {
				t.Fatalf("row does not fill width %v", width)
			}
			for i := start + 1; i <= end; i++ {
				gap := objects[i].Position().X - objects[i-1].Position().X - objects[i-1].Size().Width
				if math.Abs(float64(gap-theme.Padding()*3)) > 0.01 || objects[i].Size().Width != objects[start].Size().Width {
					t.Fatalf("row has uneven widths or gaps at width %v", width)
				}
			}
			start = end + 1
		}
		last := objects[len(objects)-1]
		if last.Size().Height < 320 || max(float32(600), layout.MinSize(objects).Height) < last.Position().Y+last.Size().Height {
			t.Fatal("scrollable height does not include every row")
		}
	}
	objects := []fyne.CanvasObject{canvas.NewRectangle(nil)}
	layout := &categoryGridLayout{cellSize: fyne.NewSize(340, 320)}
	layout.Layout(objects, fyne.NewSize(760, 600))
	if objects[0].Size() != fyne.NewSize(760, 600) {
		t.Fatal("single category panel did not stretch to fill the available space")
	}
}

func TestCategoryHeadingFitsAndTruncates(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	icon := newPointerButton("", theme.ContentAddIcon(), nil)
	label := widget.NewLabelWithStyle("Life", fyne.TextAlignCenter, fyne.TextStyle{Bold: true})
	label.Truncation = fyne.TextTruncateEllipsis
	heading := container.New(&categoryHeadingLayout{icon: icon, label: label}, icon, label)
	heading.Resize(fyne.NewSize(220, 40))
	if label.Size().Width < fyne.MeasureText(label.Text, theme.TextSize(), label.TextStyle).Width+theme.InnerPadding()*4 {
		t.Fatal("category title is truncated despite sufficient space")
	}
	left, right := icon.Position().X, label.Position().X+label.Size().Width
	if math.Abs(float64(left-(220-right))) > 0.01 {
		t.Fatal("category icon and title are not centered together")
	}
	heading.Resize(fyne.NewSize(60, 40))
	if label.Position().X+label.Size().Width > 60.01 {
		t.Fatal("category title exceeds available space")
	}
}

func TestCategoryGridResizesInScroll(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	w := a.NewWindow("Scrivi")
	defer w.Close()
	db := newTestDB(t)
	for _, name := range []string{"Life", "Work", "Home", "Other"} {
		if err := db.addCategoryWithIcon(name, ""); err != nil {
			t.Fatal(err)
		}
	}
	content := todoUI(w, db)
	w.SetContent(content)
	w.Show()
	scroll := findUIObject(t, content, func(scroll *container.Scroll) bool {
		group, ok := scroll.Content.(*fyne.Container)
		if !ok {
			return false
		}
		_, ok = group.Layout.(*categoryGridLayout)
		return ok
	})
	for _, width := range []float32{1200, 400, 760, 1200} {
		w.Resize(fyne.NewSize(width, 600))
		grid := scroll.Content.(*fyne.Container)
		last := grid.Objects[len(grid.Objects)-1]
		if last.Position().Y+last.Size().Height > grid.Size().Height+0.01 {
			t.Fatalf("last category is outside the scroll content after resizing to %v", width)
		}
		if math.Abs(float64(last.Position().X+last.Size().Width-grid.Size().Width)) > 0.01 {
			t.Fatalf("last row does not fill the grid after resizing to %v", width)
		}
	}
}

func TestButtonAndTitleCursors(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	button := newPointerButton("Add", nil, nil)
	if button.Cursor() != desktop.PointerCursor {
		t.Fatal("button does not have a pointer cursor")
	}
	button.Disable()
	if button.Cursor() != desktop.DefaultCursor {
		t.Fatal("disabled button has an active pointer cursor")
	}
	check := newPointerCheck()
	if check.Cursor() != desktop.PointerCursor {
		t.Fatal("checkbox does not have a pointer cursor")
	}
	title := newTodoTitle("Task", false, func() {}).(*todoTitleWidget)
	if title.Cursor() != desktop.PointerCursor {
		t.Fatal("clickable title does not have a pointer cursor")
	}
}
