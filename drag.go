package main

import (
	"time"

	"fyne.io/fyne/v2"
	"fyne.io/fyne/v2/container"
	"fyne.io/fyne/v2/driver/desktop"
	"fyne.io/fyne/v2/theme"
	"fyne.io/fyne/v2/widget"
)

var dragIcon = theme.NewThemedResource(fyne.NewStaticResource("drag.svg", []byte(
	`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g fill="#000000"><circle cx="9" cy="5" r="1.6"/><circle cx="15" cy="5" r="1.6"/><circle cx="9" cy="12" r="1.6"/><circle cx="15" cy="12" r="1.6"/><circle cx="9" cy="19" r="1.6"/><circle cx="15" cy="19" r="1.6"/></g></svg>`)))

type dragHandle struct {
	widget.BaseWidget
	icon   *widget.Icon
	last   fyne.Position
	active bool
	move   func(fyne.Position)
	end    func(fyne.Position)
}

func newDragHandle(move, end func(fyne.Position)) *dragHandle {
	h := &dragHandle{icon: widget.NewIcon(dragIcon), move: move, end: end}
	h.ExtendBaseWidget(h)
	return h
}

func (h *dragHandle) CreateRenderer() fyne.WidgetRenderer { return widget.NewSimpleRenderer(h.icon) }
func (h *dragHandle) Cursor() desktop.Cursor              { return desktop.PointerCursor }
func (h *dragHandle) Dragged(event *fyne.DragEvent) {
	h.active = true
	h.last = event.AbsolutePosition
	h.move(h.last)
}
func (h *dragHandle) DragEnd() {
	if h.active {
		h.active = false
		h.end(h.last)
	}
}

type dragController struct {
	window   fyne.Window
	db       *memoryDB
	scroll   *container.Scroll
	panels   map[int]*todoCategoryPanel
	refresh  func()
	hovered  *categorySurface
	last     fyne.Position
	stop     chan struct{}
	sequence uint64
}

func (d *dragController) contains(object fyne.CanvasObject, point fyne.Position) bool {
	pos := fyne.CurrentApp().Driver().AbsolutePositionForObject(object)
	size := object.Size()
	return point.X >= pos.X && point.Y >= pos.Y && point.X < pos.X+size.Width && point.Y < pos.Y+size.Height
}

func (d *dragController) target(point fyne.Position) *todoCategoryPanel {
	if !d.contains(d.scroll, point) {
		return nil
	}
	for _, group := range d.db.listCategories() {
		panel := d.panels[group.ID]
		if panel != nil && d.contains(panel.content, point) {
			return panel
		}
	}
	return nil
}

func (d *dragController) scrollNear(point fyne.Position) {
	if d.contains(d.scroll, point) {
		pos := fyne.CurrentApp().Driver().AbsolutePositionForObject(d.scroll)
		before := d.scroll.Offset.Y
		if point.Y < pos.Y+24 {
			d.scroll.Scrolled(&fyne.ScrollEvent{Scrolled: fyne.NewDelta(0, 12)})
		} else if point.Y > pos.Y+d.scroll.Size().Height-24 {
			d.scroll.Scrolled(&fyne.ScrollEvent{Scrolled: fyne.NewDelta(0, -12)})
		}
		if d.scroll.Offset.Y != before {
			return
		}
	}
	panel := d.target(point)
	if panel != nil && d.contains(panel.scroll, point) {
		pos := fyne.CurrentApp().Driver().AbsolutePositionForObject(panel.scroll)
		if point.Y < pos.Y+24 {
			panel.scroll.Scroll.Scrolled(&fyne.ScrollEvent{Scrolled: fyne.NewDelta(0, 12)})
		} else if point.Y > pos.Y+panel.scroll.Size().Height-24 {
			panel.scroll.Scroll.Scrolled(&fyne.ScrollEvent{Scrolled: fyne.NewDelta(0, -12)})
		}
	}
}

func (d *dragController) hover(point fyne.Position) {
	d.last = point
	if d.stop == nil {
		d.sequence++
		sequence := d.sequence
		stop := make(chan struct{})
		d.stop = stop
		go func() {
			ticker := time.NewTicker(60 * time.Millisecond)
			defer ticker.Stop()
			for {
				select {
				case <-stop:
					return
				case <-ticker.C:
					fyne.Do(func() {
						if d.stop != nil && d.sequence == sequence {
							d.scrollNear(d.last)
							d.updateHover(d.last)
						}
					})
				}
			}
		}()
	}
	d.updateHover(point)
}

func (d *dragController) updateHover(point fyne.Position) {
	panel := d.target(point)
	var surface *categorySurface
	if panel != nil {
		surface = panel.surface
	}
	if d.hovered == surface {
		return
	}
	d.clear()
	d.hovered = surface
	if surface != nil {
		surface.highlighted = true
		surface.Refresh()
	}
}

func (d *dragController) finish() {
	if d.stop != nil {
		close(d.stop)
		d.stop = nil
	}
	d.clear()
}

func (d *dragController) clear() {
	if d.hovered != nil {
		d.hovered.highlighted = false
		d.hovered.Refresh()
		d.hovered = nil
	}
}

func (d *dragController) dropCategory(id int, point fyne.Position) {
	panel := d.target(point)
	d.finish()
	if panel == nil || panel.id == id {
		return
	}
	position := fyne.CurrentApp().Driver().AbsolutePositionForObject(panel.content)
	after := point.X >= position.X+panel.content.Size().Width/2
	if grid, ok := d.scroll.Content.(*fyne.Container); ok {
		if layout, ok := grid.Layout.(*categoryGridLayout); ok && layout.columns == 1 {
			after = point.Y >= position.Y+panel.content.Size().Height/2
		}
	}
	index := 0
	for _, group := range d.db.listCategories() {
		if group.ID == id {
			continue
		}
		if group.ID == panel.id {
			if after {
				index++
			}
			break
		}
		index++
	}
	if err := d.db.moveCategory(id, index); err != nil {
		showError(err, d.window)
		return
	}
	d.refresh()
}

func (d *dragController) dropTodo(id int, point fyne.Position) {
	panel := d.target(point)
	d.finish()
	if panel == nil {
		return
	}
	index := 0
	for _, row := range panel.itemRows {
		if row.id == id {
			continue
		}
		position := fyne.CurrentApp().Driver().AbsolutePositionForObject(row.content)
		if point.Y < position.Y+row.content.Size().Height/2 {
			break
		}
		index++
	}
	if err := d.db.moveTodo(id, panel.id, index); err != nil {
		showError(err, d.window)
		return
	}
	d.refresh()
}
