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
