package main

import (
	"testing"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/test"
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

	findButton := func(root fyne.CanvasObject, text string) *widget.Button {
		t.Helper()
		return findUIObject(t, root, func(button *widget.Button) bool { return button.Text == text })
	}
	findPanel := func(name string) *widget.Card {
		t.Helper()
		return findUIObject(t, content, func(card *widget.Card) bool {
			header := card.Content.(*fyne.Container).Objects[1].(*fyne.Container)
			title := header.Objects[0].(*fyne.Container).Objects[0].(*widget.Label)
			return title.Text == name
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
	test.Tap(findButton(content, "Add category"))
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
	findUIObject(t, uncategorized, func(label *widget.Label) bool { return label.Text == "Finish report" })
	if draft.Text != "Keep this draft" || findTodoEntry(life).Text != "Another draft" {
		t.Fatal("deleting a category discarded another panel's draft")
	}
	test.Tap(findButton(uncategorized, ""))
	if len(db.list()) != 0 {
		t.Fatal("moved todo could not be deleted")
	}
}
