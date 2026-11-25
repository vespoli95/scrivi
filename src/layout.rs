use egui::{pos2, vec2, Pos2, Rect, Vec2};

pub const CELL_MIN_WIDTH: f32 = 340.0;
pub const CELL_MIN_HEIGHT: f32 = 240.0;
pub const GRID_GAP: f32 = 12.0;
pub const DEFAULT_WINDOW_WIDTH: f32 = 760.0;
pub const DEFAULT_WINDOW_HEIGHT: f32 = 600.0;
pub const PINNED_WINDOW_WIDTH: f32 = 380.0;
pub const PINNED_WINDOW_HEIGHT: f32 = 360.0;

#[derive(Clone, Debug, PartialEq)]
pub struct CategoryGridLayout {
    pub columns: usize,
    pub rows: usize,
    pub cell_height: f32,
    pub content_height: f32,
    pub rects: Vec<Rect>,
}

/// Computes responsive grid positions for category cards, stretching widths and heights
/// to fill available space while guaranteeing minimum cell dimensions.
pub fn compute_category_grid(
    count: usize,
    avail_size: Vec2,
    cell_min_size: Vec2,
    gap: f32,
) -> CategoryGridLayout {
    let columns = (((avail_size.x + gap) / (cell_min_size.x + gap)).floor() as isize).max(1) as usize;
    let rows = if count == 0 {
        1
    } else {
        count.div_ceil(columns)
    };
    let mut cell_height = cell_min_size.y;
    if count > 0 {
        let stretched = (avail_size.y - (rows as f32 - 1.0) * gap) / (rows as f32);
        cell_height = cell_height.max(stretched);
    }
    let min_height = (rows as f32) * cell_min_size.y + (rows as f32 - 1.0) * gap;
    let mut rects = Vec::with_capacity(count);
    let mut max_bottom = 0.0_f32;

    let mut start = 0;
    while start < count {
        let row_count = (count - start).min(columns);
        let width = ((avail_size.x - (row_count as f32 - 1.0) * gap) / (row_count as f32)).max(0.0);
        let row_index = start / columns;
        let y = (row_index as f32) * (cell_height + gap);
        for col in 0..row_count {
            let x = (col as f32) * (width + gap);
            let rect = Rect::from_min_size(pos2(x, y), vec2(width, cell_height));
            max_bottom = max_bottom.max(rect.bottom());
            rects.push(rect);
        }
        start += columns;
    }

    CategoryGridLayout {
        columns,
        rows,
        cell_height,
        content_height: min_height.max(max_bottom),
        rects,
    }
}

/// Stacks categories in a single vertical column where each card fills the visible viewport
/// height so pinned mode shows the first category while remaining scrollable to others.
pub fn compute_pinned_category_stack(
    count: usize,
    avail_size: Vec2,
    cell_min_height: f32,
    gap: f32,
) -> CategoryGridLayout {
    let columns = 1;
    let rows = count.max(1);
    let cell_height = avail_size.y.max(cell_min_height);
    let width = avail_size.x.max(0.0);
    let mut rects = Vec::with_capacity(count);
    for idx in 0..count {
        let y = (idx as f32) * (cell_height + gap);
        rects.push(Rect::from_min_size(pos2(0.0, y), vec2(width, cell_height)));
    }
    let content_height = if count == 0 {
        cell_height
    } else {
        (count as f32) * cell_height + (count as f32 - 1.0) * gap
    };

    CategoryGridLayout {
        columns,
        rows,
        cell_height,
        content_height,
        rects,
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeadingLayout {
    pub icon_rect: Rect,
    pub label_rect: Rect,
}

/// Centers a category icon and title together within `avail_rect`, clamping label width
/// so it truncates cleanly without overflowing.
pub fn compute_heading_layout(
    avail_rect: Rect,
    icon_size: Vec2,
    text_width: f32,
    gap: f32,
) -> HeadingLayout {
    let max_label_width = (avail_rect.width() - icon_size.x - gap).max(0.0);
    let label_width = text_width.min(max_label_width);
    let total_width = icon_size.x + gap + label_width;
    let start_x = avail_rect.left() + ((avail_rect.width() - total_width) * 0.5).max(0.0);
    let icon_y = avail_rect.top() + (avail_rect.height() - icon_size.y) * 0.5;
    let icon_rect = Rect::from_min_size(pos2(start_x, icon_y), icon_size);
    let label_rect = Rect::from_min_size(
        pos2(start_x + icon_size.x + gap, avail_rect.top()),
        vec2(label_width, avail_rect.height()),
    );
    HeadingLayout {
        icon_rect,
        label_rect,
    }
}

pub fn compute_category_drop_index(
    ordered_ids: &[i64],
    dragged_id: i64,
    target_id: i64,
    target_rect: Rect,
    point: Pos2,
    columns: usize,
) -> usize {
    let after = if columns == 1 {
        point.y >= target_rect.center().y
    } else {
        point.x >= target_rect.center().x
    };
    let mut index = 0;
    for &id in ordered_ids {
        if id == dragged_id {
            continue;
        }
        if id == target_id {
            if after {
                index += 1;
            }
            break;
        }
        index += 1;
    }
    index
}

pub fn compute_todo_drop_index(row_rects: &[(i64, Rect)], dragged_id: i64, point: Pos2) -> usize {
    let mut index = 0;
    for &(id, rect) in row_rects {
        if id == dragged_id {
            continue;
        }
        if point.y < rect.center().y {
            break;
        }
        index += 1;
    }
    index
}

/// Horizontal indentation applied to todos nested under a sub-category.
pub const SUBTODO_INDENT: f32 = 28.0;

/// A rendered todo row, in visual order, used for nested drop hit-testing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TodoDropRow {
    pub id: i64,
    pub rect: Rect,
    pub parent_id: Option<i64>,
    pub is_group: bool,
}

/// Where a dragged todo lands: `index` among the siblings under `parent_id`
/// (a sub-category), or among the top-level items when `parent_id` is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TodoDropTarget {
    pub parent_id: Option<i64>,
    pub index: usize,
}

/// Resolves a drop point against a category's flattened rows.
///
/// * Dropping onto the middle of a sub-category heading appends into it.
/// * Dropping between nested rows inserts into that sub-category.
/// * Dropping just below a sub-category's last row nests when the pointer is
///   indented to at least `nest_x`; otherwise the todo lands at the top level.
/// * Sub-categories themselves always land at the top level.
pub fn compute_nested_todo_drop(
    rows: &[TodoDropRow],
    dragged_id: i64,
    dragging_group: bool,
    point: Pos2,
    nest_x: f32,
) -> TodoDropTarget {
    let visible: Vec<&TodoDropRow> = rows
        .iter()
        .filter(|r| r.id != dragged_id && r.parent_id != Some(dragged_id))
        .collect();

    if !dragging_group {
        for row in visible.iter().filter(|r| r.is_group) {
            let band = row.rect.height() * 0.25;
            if point.y >= row.rect.top() + band && point.y <= row.rect.bottom() - band {
                let index = visible
                    .iter()
                    .filter(|r| r.parent_id == Some(row.id))
                    .count();
                return TodoDropTarget {
                    parent_id: Some(row.id),
                    index,
                };
            }
        }
    }

    let slot = visible
        .iter()
        .position(|r| point.y < r.rect.center().y)
        .unwrap_or(visible.len());
    let above = slot.checked_sub(1).map(|i| visible[i]);
    let below = visible.get(slot);

    let parent_id = if dragging_group {
        None
    } else if let Some(group) = below.and_then(|r| r.parent_id) {
        Some(group)
    } else {
        match above.and_then(|r| if r.is_group { Some(r.id) } else { r.parent_id }) {
            Some(group) if point.x >= nest_x => Some(group),
            _ => None,
        }
    };
    let index = visible[..slot]
        .iter()
        .filter(|r| r.parent_id == parent_id)
        .count();
    TodoDropTarget { parent_id, index }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: i64, y: f32, parent_id: Option<i64>, is_group: bool) -> TodoDropRow {
        let left = if parent_id.is_some() { SUBTODO_INDENT } else { 0.0 };
        TodoDropRow {
            id,
            rect: Rect::from_min_max(pos2(left, y), pos2(300.0, y + 20.0)),
            parent_id,
            is_group,
        }
    }

    // A(1) top, G(2) group with children C1(3), C2(4), B(5) top
    fn sample() -> Vec<TodoDropRow> {
        vec![
            row(1, 0.0, None, false),
            row(2, 24.0, None, true),
            row(3, 48.0, Some(2), false),
            row(4, 72.0, Some(2), false),
            row(5, 96.0, None, false),
        ]
    }

    fn target(parent_id: Option<i64>, index: usize) -> TodoDropTarget {
        TodoDropTarget { parent_id, index }
    }

    #[test]
    fn test_nested_todo_drop_targets() {
        let rows = sample();
        let nest_x = SUBTODO_INDENT;
        // Onto group heading middle -> append into group
        assert_eq!(compute_nested_todo_drop(&rows, 5, false, pos2(10.0, 34.0), nest_x), target(Some(2), 2));
        // Between children -> into group
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(10.0, 70.0), nest_x), target(Some(2), 1));
        // Upper edge of group heading -> top level before group
        assert_eq!(compute_nested_todo_drop(&rows, 5, false, pos2(10.0, 25.0), nest_x), target(None, 1));
        // Below last child, not indented -> top level after group
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(10.0, 94.0), nest_x), target(None, 1));
        // Below last child, indented -> end of group
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(40.0, 94.0), nest_x), target(Some(2), 2));
        // Child dragged out to the very top
        assert_eq!(compute_nested_todo_drop(&rows, 3, false, pos2(10.0, 2.0), nest_x), target(None, 0));
        // Groups never nest: dropping a group amid children lands after its host group
        let mut with_other = rows.clone();
        with_other.push(row(6, 120.0, None, true));
        assert_eq!(compute_nested_todo_drop(&with_other, 6, true, pos2(40.0, 70.0), nest_x), target(None, 2));
        assert_eq!(compute_nested_todo_drop(&with_other, 6, true, pos2(40.0, 34.0), nest_x), target(None, 2));
        // Dragging a group ignores its own children
        assert_eq!(compute_nested_todo_drop(&rows, 2, true, pos2(10.0, 200.0), nest_x), target(None, 2));
        // Empty list
        assert_eq!(compute_nested_todo_drop(&[], 9, false, pos2(0.0, 0.0), nest_x), target(None, 0));
    }
}
