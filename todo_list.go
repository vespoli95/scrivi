package main

import (
	"strings"
	"unicode"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/canvas"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/driver/desktop"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

func wrapTodoText(text string, width float32) []string {
	var lines []string
	for _, paragraph := range strings.Split(text, "\n") {
		runes := []rune(paragraph)
		if len(runes) == 0 {
			lines = append(lines, "")
		}
		for len(runes) > 0 {
			end, lastSpace := 0, -1
			for end < len(runes) {
				if end > 0 && fyne.MeasureText(string(runes[:end+1]), theme.TextSize(), fyne.TextStyle{}).Width > width {
					break
				}
				if unicode.IsSpace(runes[end]) {
					lastSpace = end
				}
				end++
			}
			if end < len(runes) && lastSpace > 0 {
				end = lastSpace
			}
			lines = append(lines, string(runes[:end]))
			runes = runes[end:]
			for len(runes) > 0 && unicode.IsSpace(runes[0]) {
				runes = runes[1:]
			}
		}
	}
	return lines
}

type todoTitleWidget struct {
	widget.BaseWidget
	content  *fyne.Container
	title    string
	done     bool
	onTapped func()
}

func newTodoTitle(title string, done bool, onTapped func()) fyne.CanvasObject {
	t := &todoTitleWidget{content: container.NewWithoutLayout(), title: title, done: done, onTapped: onTapped}
	t.ExtendBaseWidget(t)
	return t
}

func (t *todoTitleWidget) CreateRenderer() fyne.WidgetRenderer {
	return widget.NewSimpleRenderer(t.content)
}

func (t *todoTitleWidget) heightFor(width float32) float32 {
	lineHeight := fyne.MeasureText("M", theme.TextSize(), fyne.TextStyle{}).Height
	return float32(len(wrapTodoText(t.title, max(1, width-theme.InnerPadding()*2))))*lineHeight + theme.InnerPadding()*2
}

func (t *todoTitleWidget) Resize(size fyne.Size) {
	t.BaseWidget.Resize(size)
	t.render()
}

func (t *todoTitleWidget) Refresh() {
	t.render()
	t.BaseWidget.Refresh()
}

func (t *todoTitleWidget) render() {
	if t.Size().Width <= 0 {
		return
	}
	lines := wrapTodoText(t.title, max(1, t.Size().Width-theme.InnerPadding()*2))
	lineHeight := fyne.MeasureText("M", theme.TextSize(), fyne.TextStyle{}).Height
	startY := max(theme.InnerPadding(), (t.Size().Height-float32(len(lines))*lineHeight)/2)
	ink := theme.ForegroundColor()
	if t.done {
		ink = theme.DisabledColor()
	}
	objects := make([]fyne.CanvasObject, 0, len(lines)*2)
	for i, value := range lines {
		text := canvas.NewText(value, ink)
		text.TextSize = theme.TextSize()
		pos := fyne.NewPos(theme.InnerPadding(), startY+float32(i)*lineHeight)
		text.Move(pos)
		text.Resize(text.MinSize())
		objects = append(objects, text)
		if t.done && value != "" {
			line := canvas.NewLine(ink)
			line.Move(fyne.NewPos(pos.X, pos.Y+lineHeight/2))
			line.Resize(fyne.NewSize(text.MinSize().Width, 0))
			objects = append(objects, line)
		}
	}
	t.content.Objects = objects
	t.content.Refresh()
}

func (t *todoTitleWidget) Tapped(*fyne.PointEvent) { t.onTapped() }
func (t *todoTitleWidget) Cursor() desktop.Cursor  { return desktop.PointerCursor }

type todoRow struct {
	id          int
	content     *fyne.Container
	title       *todoTitleWidget
	left, right fyne.CanvasObject
}

func (r *todoRow) heightFor(width float32) float32 {
	titleWidth := max(1, width-r.left.MinSize().Width-r.right.MinSize().Width-2*theme.Padding())
	return max(r.left.MinSize().Height, r.right.MinSize().Height, r.title.heightFor(titleWidth))
}

type todoRowsLayout struct {
	rows  []*todoRow
	width float32
}

func (l *todoRowsLayout) Layout(objects []fyne.CanvasObject, size fyne.Size) {
	l.width = size.Width
	y := float32(0)
	for _, row := range l.rows {
		height := row.heightFor(size.Width)
		row.content.Move(fyne.NewPos(0, y))
		row.content.Resize(fyne.NewSize(size.Width, height))
		y += height + theme.Padding()
	}
	if len(l.rows) == 0 && len(objects) > 0 {
		objects[0].Resize(fyne.NewSize(size.Width, objects[0].MinSize().Height))
	}
}

func (l *todoRowsLayout) MinSize(objects []fyne.CanvasObject) fyne.Size {
	height := float32(0)
	for _, row := range l.rows {
		if height > 0 {
			height += theme.Padding()
		}
		height += row.heightFor(max(l.width, 1))
	}
	if len(l.rows) == 0 && len(objects) > 0 {
		height = objects[0].MinSize().Height
	}
	return fyne.NewSize(1, height)
}

type nestedScroll struct {
	*container.Scroll
	parent *container.Scroll
	rows   *todoRowsLayout
}

func newNestedScroll(content fyne.CanvasObject, rows *todoRowsLayout, parent *container.Scroll) *nestedScroll {
	s := &nestedScroll{Scroll: &container.Scroll{Content: content, Direction: container.ScrollVerticalOnly}, parent: parent, rows: rows}
	s.ExtendBaseWidget(s)
	return s
}

func (s *nestedScroll) Resize(size fyne.Size) {
	s.rows.width = size.Width
	s.Scroll.Resize(size)
}

func (s *nestedScroll) Scrolled(event *fyne.ScrollEvent) {
	before := s.Offset.Y
	s.Scroll.Scrolled(event)
	used := before - s.Offset.Y
	remaining := event.Scrolled.DY - used
	if remaining != 0 && s.parent != nil {
		s.parent.Scrolled(&fyne.ScrollEvent{PointEvent: event.PointEvent, Scrolled: fyne.NewDelta(event.Scrolled.DX, remaining)})
	}
}
