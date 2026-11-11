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
	test.Tap(findButton(content, "Emoji"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "🏠"))
	test.Tap(findButton(content, "Add category"))
	if db.listCategories()[0].Icon != "🏠" || findButton(content, "Emoji") == nil {
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
	heading := findUIObject(t, life, func(label *widget.Label) bool { return label.Text == "🏠 Life" })
	headingPosition := a.Driver().AbsolutePositionForObject(heading)
	panelPosition := a.Driver().AbsolutePositionForObject(life)
	if math.Abs(float64(headingPosition.X+heading.Size().Width/2-panelPosition.X-life.Size().Width/2)) > 0.01 {
		t.Fatal("category heading is not centered in its panel")
	}
	if life.Size().Width <= 340 || life.Size().Height <= 240 {
		t.Fatal("category panel does not stretch to fill the available window height")
	}
	findTodoEntry(life).SetText("Another draft")
	test.Tap(findButton(life, "Icon"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "⭐"))
	if db.listCategories()[0].Icon != "⭐" || findTodoEntry(life).Text != "Another draft" {
		t.Fatal("changing the category emoji lost its draft or did not save")
	}
	findUIObject(t, life, func(label *widget.Label) bool {
		return label.Text == "⭐ Life" && label.Alignment == fyne.TextAlignCenter
	})
	test.Tap(findButton(life, "Icon"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "Cancel"))
	if db.listCategories()[0].Icon != "⭐" {
		t.Fatal("canceling the emoji picker changed the category")
	}
	test.Tap(findButton(life, "Icon"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "No icon"))
	if db.listCategories()[0].Icon != "" {
		t.Fatal("category emoji could not be removed")
	}
	entry := findTodoEntry(work)
	entry.SetText("Finish report")
	test.Tap(findButton(work, "Add"))
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
	test.Tap(findButton(workHeader, ""))
	findUIObject(t, w.Canvas().Overlays().Top(), func(label *widget.Label) bool {
		return strings.Contains(label.Text, "all its todos")
	})
	test.Tap(findButton(w.Canvas().Overlays().Top(), "No"))
	if len(db.listCategories()) != 2 || len(db.list()) != 1 {
		t.Fatal("canceling category deletion removed the category")
	}
	test.Tap(findButton(workHeader, ""))
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
	button := newOptionsButton(w, func() (bool, error) {
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

func TestTodoStrikethroughFollowsRenderedText(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	for _, title := range []string{"Buy milk", "A very long completed todo that must be shortened", "Résumé 日本語"} {
		for _, width := range []float32{200, 80, 1} {
			content := newTodoTitle(title, true, func() {}).(*todoTitleWidget).content.(*fyne.Container)
			content.Resize(fyne.NewSize(width, 40))
			text := content.Objects[0].(*canvas.Text)
			line := content.Objects[1].(*canvas.Line)
			if line.Position1.X != text.Position().X || line.Position1.Y != 20 {
				t.Fatalf("strike does not start on the text: %q, %+v", title, line.Position1)
			}
			if math.Abs(float64(line.Position2.X-(text.Position().X+text.MinSize().Width))) > 0.01 {
				t.Fatalf("strike does not end on the rendered text: %q", text.Text)
			}
			if text.Text == "" {
				if line.Visible() {
					t.Fatal("empty text still has a visible strike")
				}
				continue
			}
			if text.MinSize().Width > width-theme.InnerPadding()*2 {
				t.Fatalf("rendered text exceeds available width: %q", text.Text)
			}
			if title != text.Text && !strings.HasSuffix(text.Text, "…") {
				t.Fatal("shortened title is missing its ellipsis")
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
