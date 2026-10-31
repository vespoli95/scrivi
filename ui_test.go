package main

import (
	"image/color"
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
	uncategorized := findPanel("Uncategorized")
	draft := findTodoEntry(uncategorized)
	draft.SetText("Keep this draft")
	categoryEntry := findUIObject(t, content, func(entry *widget.Entry) bool {
		return entry.PlaceHolder == "Category name (Life, Money, Work...)"
	})
	categoryEntry.SetText("Life")
	test.Tap(findButton(content, "Emoji"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "🏠"))
	test.Tap(findButton(content, "Add category"))
	if db.listCategories()[1].Icon != "🏠" || findButton(content, "Emoji") == nil {
		t.Fatal("category emoji was not saved and reset")
	}
	categoryEntry.SetText("Work")
	categoryEntry.OnSubmitted(categoryEntry.Text)
	if len(db.listCategories()) != 3 || categoryEntry.Text != "" {
		t.Fatal("category controls did not add and clear the input")
	}
	if draft.Text != "Keep this draft" || findPanel("Uncategorized") != uncategorized {
		t.Fatal("adding a category discarded an existing panel's draft")
	}
	life := findPanel("Life")
	work := findPanel("Work")
	heading := findUIObject(t, life, func(label *widget.Label) bool { return label.Text == "🏠 Life" })
	headingPosition := a.Driver().AbsolutePositionForObject(heading)
	panelPosition := a.Driver().AbsolutePositionForObject(life)
	if math.Abs(float64(headingPosition.X+heading.Size().Width/2-panelPosition.X-life.Size().Width/2)) > 0.01 {
		t.Fatal("category heading is not centered in its panel")
	}
	frame := findUIObject(t, content, func(rectangle *canvas.Rectangle) bool {
		return rectangle.StrokeColor == color.White && rectangle.StrokeWidth > 0
	})
	if frame.Size().Width != 340 || frame.Size().Height != 320 {
		t.Fatal("white border does not enclose the category panel")
	}
	findTodoEntry(life).SetText("Another draft")
	test.Tap(findButton(life, "Icon"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "⭐"))
	if db.listCategories()[1].Icon != "⭐" || findTodoEntry(life).Text != "Another draft" {
		t.Fatal("changing the category emoji lost its draft or did not save")
	}
	findUIObject(t, life, func(label *widget.Label) bool {
		return label.Text == "⭐ Life" && label.Alignment == fyne.TextAlignCenter
	})
	test.Tap(findButton(life, "Icon"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "Cancel"))
	if db.listCategories()[1].Icon != "⭐" {
		t.Fatal("canceling the emoji picker changed the category")
	}
	test.Tap(findButton(life, "Icon"))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "No icon"))
	if db.listCategories()[1].Icon != "" {
		t.Fatal("category emoji could not be removed")
	}
	entry := findTodoEntry(work)
	entry.SetText("Finish report")
	test.Tap(findButton(work, "Add"))
	item := db.list()[0]
	if item.Title != "Finish report" || item.CategoryID != db.listCategories()[2].ID || entry.Text != "" {
		t.Fatalf("todo was not added to Work: %+v", item)
	}
	title := findUIObject(t, work, func(title *todoTitleWidget) bool { return true })
	test.Tap(title)
	if !db.list()[0].Done {
		t.Fatal("category todo could not be completed by tapping its title")
	}
	findTodoEntry(life).SetText("Another draft")
	workHeader := work.Content.(*fyne.Container).Objects[1]
	test.Tap(findButton(workHeader, ""))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "No"))
	if len(db.listCategories()) != 3 {
		t.Fatal("canceling category deletion removed the category")
	}
	test.Tap(findButton(workHeader, ""))
	test.Tap(findButton(w.Canvas().Overlays().Top(), "Yes"))
	if len(db.listCategories()) != 2 || db.list()[0].CategoryID != 0 || !db.list()[0].Done {
		t.Fatal("category deletion did not preserve and move its todo")
	}
	findUIObject(t, uncategorized, func(text *canvas.Text) bool { return text.Text == "Finish report" })
	if draft.Text != "Keep this draft" || findTodoEntry(life).Text != "Another draft" {
		t.Fatal("deleting a category discarded another panel's draft")
	}
	test.Tap(findButton(uncategorized, ""))
	if len(db.list()) != 0 {
		t.Fatal("moved todo could not be deleted")
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

func TestCategoryRowsAreCentered(t *testing.T) {
	a := test.NewApp()
	defer a.Quit()
	for _, width := range []float32{340, 760, 1200} {
		objects := []fyne.CanvasObject{
			canvas.NewRectangle(nil), canvas.NewRectangle(nil), canvas.NewRectangle(nil),
			canvas.NewRectangle(nil), canvas.NewRectangle(nil),
		}
		layout := &centeredCategoryLayout{cellSize: fyne.NewSize(340, 320)}
		layout.Layout(objects, fyne.NewSize(width, 600))
		for start := 0; start < len(objects); {
			end := start
			for end+1 < len(objects) && objects[end+1].Position().Y == objects[start].Position().Y {
				end++
			}
			left := objects[start].Position().X
			right := width - objects[end].Position().X - objects[end].Size().Width
			if math.Abs(float64(left-right)) > 0.01 {
				t.Fatalf("row is not centered at width %v: left=%v right=%v", width, left, right)
			}
			start = end + 1
		}
		last := objects[len(objects)-1]
		if layout.MinSize(objects).Height != last.Position().Y+last.Size().Height {
			t.Fatal("scrollable height does not include every row")
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
