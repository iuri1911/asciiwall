/// Character grid: each cell is `[glyph_index, palette_slot]`.
pub struct Grid {
    pub cols: u32,
    pub rows: u32,
    pub cells: Vec<[u8; 2]>,
}

impl Grid {
    pub fn new(cols: u32, rows: u32) -> Grid {
        Grid { cols, rows, cells: vec![[0, 0]; (cols * rows) as usize] }
    }

    pub fn bytes(&self) -> &[u8] {
        self.cells.as_flattened()
    }
}

/// Fit whole cells into a pixel area, centered: (cols, rows, off_x, off_y).
pub fn layout(width: u32, height: u32, cell_w: u32, cell_h: u32) -> (u32, u32, u32, u32) {
    let cols = (width / cell_w).max(1);
    let rows = (height / cell_h).max(1);
    (
        cols,
        rows,
        width.saturating_sub(cols * cell_w) / 2,
        height.saturating_sub(rows * cell_h) / 2,
    )
}

#[cfg(test)]
mod tests {
    #[test]
    fn layout_centers_remainder() {
        assert_eq!(super::layout(3840, 2160, 14, 24), (274, 90, 2, 0));
    }
}
