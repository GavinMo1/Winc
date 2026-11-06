//! 当前文件的大纲；仅绘制视口内的行，扫描与树展开均不在绘制中执行。

use super::*;
use crate::icons::Direction;
use winc_core::Lines;
use winc_editor::symbols::ScanLimit;
use winc_i18n::t;

pub(super) fn draw_outline(app: &mut App, frame: &mut Frame<'_>) {
    let header = app.layout.sidebar_header;
    let p = app.palette;
    ui_text(
        frame,
        app,
        header.x + 12.0,
        header,
        t!("workspace-sidebar-outline"),
        p.text,
        header,
    );
    let area = Rect::new(
        app.layout.sidebar.x + 4.0,
        header.bottom(),
        (app.layout.sidebar.w - 8.0).max(0.0),
        (app.layout.sidebar.bottom() - header.bottom()).max(0.0),
    );
    app.symbols.hits.clear();
    let note = if app.symbol_key().is_none() {
        Some(t!("workspace-no-open-file"))
    } else if app.symbols.paused {
        Some(t!("workspace-outline-paused"))
    } else if app.symbols.error {
        Some(t!("workspace-outline-error"))
    } else if app.symbols.waiting || app.symbols.index.is_none() {
        Some(t!("workspace-outline-loading"))
    } else {
        match app.symbols.index.as_ref() {
            Some(index) if index.limit == Some(ScanLimit::Input) => {
                Some(t!("workspace-picker-symbols-too-large"))
            }
            Some(index) if index.limit.is_some() => Some(t!("workspace-outline-partial")),
            Some(index) if index.is_empty() => Some(t!("workspace-picker-no-symbols")),
            _ => None,
        }
    };
    let row_h = crate::theme::TREE_ROW;
    let top = if let Some(note) = note {
        let row = Rect::new(area.x, area.y, area.w, row_h);
        let text = ellipsize(frame, note, app.fonts.ui_small, (area.w - 16.0).max(0.0));
        ui_text_small(frame, app, area.x + 8.0, row, &text, p.text_dim, area);
        area.y + row_h
    } else {
        area.y
    };
    let available = (area.bottom() - top).max(0.0);
    let max_scroll = (app.symbols.visible.len() as f32 * row_h - available).max(0.0);
    app.symbols.scroll = app.symbols.scroll.clamp(0.0, max_scroll);
    let first = (app.symbols.scroll / row_h).floor() as usize;
    let count = (available / row_h).ceil() as usize + 1;
    let caret = app
        .editor
        .active()
        .map(|d| d.text().pos_to_line_col(d.selection.head).0);
    let current =
        app.symbols.index.as_ref().and_then(|s| {
            caret.and_then(|line| s.partition_point(|s| s.line <= line).checked_sub(1))
        });
    let valid = !app.symbols.paused && !app.symbols.waiting && app.symbols.key == app.symbol_key();
    let Some(index) = app.symbols.index.as_ref() else {
        return;
    };
    let mut hits = Vec::new();
    for visual in first..(first + count).min(app.symbols.visible.len()) {
        let i = app.symbols.visible[visual];
        let symbol = &index[i];
        let row = Rect::new(
            area.x,
            top + visual as f32 * row_h - app.symbols.scroll,
            area.w,
            row_h,
        );
        let clip = row.intersect(Rect::new(area.x, top, area.w, available));
        if current == Some(i) {
            frame.rounded(clip, p.accent.alpha(0.12), 3.0);
        } else if app.hover == crate::app::Hover::Outline(i) {
            frame.rounded(clip, p.hover, 3.0);
        }
        let x = row.x + 8.0 + symbol.depth.min(12) as f32 * 12.0;
        let has_children = index.get(i + 1).is_some_and(|next| next.parent == Some(i));
        let chevron = Rect::new(x, row.y, 16.0, row_h).intersect(clip);
        if has_children {
            let direction = if app.symbols.collapsed.contains(&i) {
                Direction::Right
            } else {
                Direction::Down
            };
            icons::chevron(
                frame,
                (x + 7.0, row.y + row_h / 2.0),
                6.0,
                direction,
                p.text_dim,
                chevron,
            );
            if valid {
                hits.push((chevron, (i, true)));
            }
        }
        let label = ellipsize(
            frame,
            &symbol.name,
            app.fonts.ui,
            (row.right() - x - 25.0).max(0.0),
        );
        ui_text(frame, app, x + 20.0, row, &label, p.text, clip);
        if valid {
            hits.push((clip, (i, false)));
        }
    }
    app.symbols.hits = hits;
}
