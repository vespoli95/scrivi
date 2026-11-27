use egui::{pos2, vec2, Pos2, Rect, Vec2};

pub const CELL_MIN_WIDTH: f32 = 340.0;
pub const CELL_MIN_HEIGHT: f32 = 240.0;
pub const CELL_MAX_HEIGHT: f32 = 2000.0;
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
    compute_category_grid_with_heights(&vec![None; count], avail_size, cell_min_size, gap)
}

/// Computes responsive grid positions for category cards while honoring optional per-card
/// height overrides. Each row advances vertically by the tallest card in that row.
pub fn compute_category_grid_with_heights(
    custom_heights: &[Option<f32>],
    avail_size: Vec2,
    cell_min_size: Vec2,
    gap: f32,
) -> CategoryGridLayout {
    let count = custom_heights.len();
    let columns = (((avail_size.x + gap) / (cell_min_size.x + gap)).floor() as isize).max(1) as usize;
    let rows = if count == 0 {
        1
    } else {
        count.div_ceil(columns)
    };
    let mut default_cell_height = cell_min_size.y;
    if count > 0 {
        let stretched = (avail_size.y - (rows as f32 - 1.0) * gap) / (rows as f32);
        default_cell_height = default_cell_height.max(stretched);
    }
    let min_height = (rows as f32) * cell_min_size.y + (rows as f32 - 1.0) * gap;
    let max_height = CELL_MAX_HEIGHT.max(cell_min_size.y);
    let mut rects = Vec::with_capacity(count);
    let mut max_bottom = 0.0_f32;

    let mut start = 0;
    let mut y = 0.0_f32;
    while start < count {
        let row_count = (count - start).min(columns);
        let width = ((avail_size.x - (row_count as f32 - 1.0) * gap) / (row_count as f32)).max(0.0);
        let mut row_height = 0.0_f32;
        for col in 0..row_count {
            let h = match custom_heights[start + col] {
                Some(custom) if custom.is_finite() => custom.clamp(cell_min_size.y, max_height),
                _ => default_cell_height,
            };
            row_height = row_height.max(h);
            let x = (col as f32) * (width + gap);
            let rect = Rect::from_min_size(pos2(x, y), vec2(width, h));
            max_bottom = max_bottom.max(rect.bottom());
            rects.push(rect);
        }
        y += row_height + gap;
        start += columns;
    }

    CategoryGridLayout {
        columns,
        rows,
        cell_height: default_cell_height,
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

/// Horizontal indentation applied to sub-items nested under another todo.
pub const SUBTODO_INDENT: f32 = 28.0;

/// A rendered todo row, in visual order, used for nested drop hit-testing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TodoDropRow {
    pub id: i64,
    pub rect: Rect,
    pub parent_id: Option<i64>,
}

/// Where a dragged todo lands: `index` among the siblings under `parent_id`
/// (a top-level todo), or among the top-level items when `parent_id` is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TodoDropTarget {
    pub parent_id: Option<i64>,
    pub index: usize,
}

/// Resolves a drop point against a category's flattened rows (max nesting depth = 1).
///
/// * Dropping onto the middle of any top-level item nests under it at the end of its sub-items.
/// * Dropping onto or between sub-items inserts into that parent's sub-items.
/// * Dropping just below an item nests under its group when the pointer is indented to at least
///   `nest_x`; dropping far below in empty space or un-indented lands at the top level.
/// * Items that currently have sub-items cannot themselves be nested and always land at top level.
pub fn compute_nested_todo_drop(
    rows: &[TodoDropRow],
    dragged_id: i64,
    dragging_has_children: bool,
    point: Pos2,
    nest_x: f32,
) -> TodoDropTarget {
    let visible: Vec<&TodoDropRow> = rows
        .iter()
        .filter(|r| r.id != dragged_id && r.parent_id != Some(dragged_id))
        .collect();

    if !dragging_has_children {
        for row in visible.iter().filter(|r| r.parent_id.is_none()) {
            let band = row.rect.height() * 0.28;
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
    let near_above = below.is_some()
        || above.is_some_and(|r| point.y <= r.rect.bottom() + r.rect.height() + 8.0);

    let parent_id = if dragging_has_children {
        None
    } else if let Some(parent) = below.and_then(|r| r.parent_id) {
        Some(parent)
    } else if near_above && point.x >= nest_x {
        above.map(|r| r.parent_id.unwrap_or(r.id))
    } else {
        None
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

    fn row(id: i64, y: f32, parent_id: Option<i64>) -> TodoDropRow {
        let left = if parent_id.is_some() { SUBTODO_INDENT } else { 0.0 };
        TodoDropRow {
            id,
            rect: Rect::from_min_max(pos2(left, y), pos2(300.0, y + 20.0)),
            parent_id,
        }
    }

    // A(1) top, G(2) top with children C1(3), C2(4), B(5) top
    fn sample() -> Vec<TodoDropRow> {
        vec![
            row(1, 0.0, None),
            row(2, 24.0, None),
            row(3, 48.0, Some(2)),
            row(4, 72.0, Some(2)),
            row(5, 96.0, None),
        ]
    }

    fn target(parent_id: Option<i64>, index: usize) -> TodoDropTarget {
        TodoDropTarget { parent_id, index }
    }

    #[test]
    fn test_nested_todo_drop_targets() {
        let rows = sample();
        let nest_x = SUBTODO_INDENT;
        // Onto middle of childless top-level item A(1) -> nests under A
        assert_eq!(compute_nested_todo_drop(&rows, 5, false, pos2(10.0, 10.0), nest_x), target(Some(1), 0));
        // Onto middle of parent item G(2) -> appends into G
        assert_eq!(compute_nested_todo_drop(&rows, 5, false, pos2(10.0, 34.0), nest_x), target(Some(2), 2));
        // Upper 25% of top-level item -> reorders above, does not nest
        assert_eq!(compute_nested_todo_drop(&rows, 5, false, pos2(10.0, 5.0), nest_x), target(None, 0));
        assert_eq!(compute_nested_todo_drop(&rows, 5, false, pos2(10.0, 29.0), nest_x), target(None, 1));
        // Onto or between sub-items -> inserts as sibling under G (never depth 2)
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(10.0, 57.0), nest_x), target(Some(2), 0));
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(10.0, 70.0), nest_x), target(Some(2), 1));
        // Below last child, not indented -> top level after G
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(10.0, 94.0), nest_x), target(None, 1));
        // Below last child, indented -> end of G
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(40.0, 94.0), nest_x), target(Some(2), 2));
        // Below childless top-level item B(5), near and indented -> nests under B
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(40.0, 120.0), nest_x), target(Some(5), 0));
        // Far below last item in empty space, even if indented -> top level
        assert_eq!(compute_nested_todo_drop(&rows, 1, false, pos2(40.0, 180.0), nest_x), target(None, 2));
        // Child dragged out to the very top
        assert_eq!(compute_nested_todo_drop(&rows, 3, false, pos2(10.0, 2.0), nest_x), target(None, 0));
        // Items with children never nest: dropping G amid or onto items stays top-level and ignores its own children
        assert_eq!(compute_nested_todo_drop(&rows, 2, true, pos2(40.0, 4.0), nest_x), target(None, 0));
        assert_eq!(compute_nested_todo_drop(&rows, 2, true, pos2(40.0, 12.0), nest_x), target(None, 1));
        assert_eq!(compute_nested_todo_drop(&rows, 2, true, pos2(40.0, 200.0), nest_x), target(None, 2));
        let with_other = vec![
            row(1, 0.0, None),
            row(2, 24.0, None),
            row(3, 48.0, Some(2)),
            row(4, 72.0, Some(2)),
            row(5, 96.0, None),
            row(6, 120.0, Some(5)),
        ];
        assert_eq!(compute_nested_todo_drop(&with_other, 5, true, pos2(40.0, 70.0), nest_x), target(None, 2));
        assert_eq!(compute_nested_todo_drop(&with_other, 5, true, pos2(40.0, 34.0), nest_x), target(None, 2));
        // Empty list
        assert_eq!(compute_nested_todo_drop(&[], 9, false, pos2(0.0, 0.0), nest_x), target(None, 0));
    }

    #[test]
    fn test_category_grid_with_custom_heights() {
        let avail = vec2(760.0, 600.0);
        let min_size = vec2(CELL_MIN_WIDTH, CELL_MIN_HEIGHT);

        // All None heights match compute_category_grid exactly
        for count in [0, 1, 2, 3, 4, 5] {
            assert_eq!(
                compute_category_grid_with_heights(&vec![None; count], avail, min_size, GRID_GAP),
                compute_category_grid(count, avail, min_size, GRID_GAP)
            );
        }

        // Extending one card in a 2-column row pushes the next row below max row height + gap
        let grid = compute_category_grid_with_heights(
            &[Some(420.0), None, Some(300.0), None],
            avail,
            min_size,
            GRID_GAP,
        );
        assert_eq!(grid.columns, 2);
        assert_eq!(grid.rects.len(), 4);
        assert!((grid.rects[0].height() - 420.0).abs() < 0.01);
        assert!((grid.rects[1].height() - grid.cell_height).abs() < 0.01);
        assert!((grid.rects[2].top() - (420.0 + GRID_GAP)).abs() < 0.01);
        assert!((grid.rects[3].top() - (420.0 + GRID_GAP)).abs() < 0.01);
        assert!((grid.rects[2].height() - 300.0).abs() < 0.01);
        assert!((grid.rects[3].height() - grid.cell_height).abs() < 0.01);
        assert!((grid.content_height - (420.0 + GRID_GAP + 300.0)).abs() < 0.01);

        // Single-column variable heights stack cleanly and update content_height
        let single_col = compute_category_grid_with_heights(
            &[Some(360.0), Some(280.0)],
            vec2(380.0, 500.0),
            min_size,
            GRID_GAP,
        );
        assert_eq!(single_col.columns, 1);
        assert!((single_col.rects[0].height() - 360.0).abs() < 0.01);
        assert!((single_col.rects[1].top() - (360.0 + GRID_GAP)).abs() < 0.01);
        assert!((single_col.rects[1].height() - 280.0).abs() < 0.01);
        assert!((single_col.content_height - (360.0 + GRID_GAP + 280.0)).abs() < 0.01);

        // Out-of-range and non-finite heights clamp or fall back safely
        let clamped = compute_category_grid_with_heights(
            &[Some(50.0), Some(9999.0), Some(f32::NAN)],
            vec2(1200.0, 600.0),
            min_size,
            GRID_GAP,
        );
        assert!((clamped.rects[0].height() - CELL_MIN_HEIGHT).abs() < 0.01);
        assert!((clamped.rects[1].height() - CELL_MAX_HEIGHT).abs() < 0.01);
        assert!((clamped.rects[2].height() - clamped.cell_height).abs() < 0.01);
    }
}
