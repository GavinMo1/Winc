//! 大纲的跨平台交互和进程级异步生命周期回归。

use super::*;
use crate::winc::Winc;

#[test]
fn outline_shares_symbols_folds_and_jumps_without_reparsing() {
    let dir = sample_folder("outline-tree");
    let file = dir.join("outline.md");
    std::fs::write(&file, "# First\n## Child\n# Second\n").unwrap();
    let mut app = app_with(&file, false);
    app.show_outline();
    settle_symbols(&mut app);
    paint(&mut app);
    assert_eq!(app.symbols.visible, [0, 1, 2]);
    let original = app.symbols.index.clone().unwrap();
    app.open_symbols();
    app.symbols_refresh();
    assert!(Arc::ptr_eq(
        &original,
        &app.picker_symbols.as_ref().unwrap().items
    ));
    app.close_picker();
    app.click_outline(0, true);
    assert_eq!(app.symbols.visible, [0, 2]);
    app.click_outline(2, false);
    let doc = app.editor.active().unwrap();
    assert_eq!(doc.text().pos_to_line_col(doc.selection.head).0, 2);
    assert!(Arc::ptr_eq(&original, app.symbols.index.as_ref().unwrap()));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn outline_only_serves_active_window_and_releases_hidden_consumers() {
    let dir = sample_folder("outline-windows");
    let file = dir.join("a.rs");
    std::fs::write(&file, "fn one() {}\n").unwrap();
    let mut winc = Winc::new(None, None);
    let a = winc.add_detached(app_with(&file, false));
    let b = winc.add_detached(app_with(&file, true));
    winc.app(a).show_outline();
    winc.app(b).show_outline();
    winc.symbols_for_test();
    assert!(winc.app(a).symbols.index.is_some());
    assert!(winc.app(b).symbols.paused && winc.app(b).symbols.index.is_none());
    winc.set_active(b);
    winc.symbols_for_test();
    assert!(winc.app(a).symbols.paused && winc.app(a).symbols.index.is_none());
    let result = winc.app(b).symbols.index.clone().unwrap();
    winc.symbols_for_test();
    assert!(Arc::ptr_eq(
        &result,
        winc.app(b).symbols.index.as_ref().unwrap()
    ));
    winc.app(b).show_files();
    winc.symbols_for_test();
    assert!(winc.app(b).symbols.index.is_none());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn editing_invalidates_outline_and_oversized_rope_never_starts_a_scan() {
    let dir = sample_folder("outline-edit");
    let file = dir.join("a.rs");
    std::fs::write(&file, "fn one() {}\n").unwrap();
    let mut winc = Winc::new(None, None);
    let key = winc.add_detached(app_with(&file, false));
    winc.app(key).show_outline();
    winc.symbols_for_test();
    winc.app(key).run(Command::Insert("fn two() {}\n".into()));
    winc.symbols_for_test();
    assert_eq!(winc.app(key).symbols.index.as_ref().unwrap().len(), 2);
    // 两次操作各小于编辑上限，累计长度超过索引输入预算，存储类型仍为 Rope。
    for _ in 0..2 {
        winc.app(key).run(Command::Insert("x".repeat(9 << 20)));
    }
    assert!(
        winc.app(key)
            .editor
            .active()
            .unwrap()
            .text()
            .rope()
            .is_some()
    );
    winc.symbols_for_test();
    assert_eq!(
        winc.app(key).symbols.index.as_ref().unwrap().limit,
        Some(winc_editor::symbols::ScanLimit::Input)
    );
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn closing_originating_window_does_not_block_next_outline() {
    let dir = sample_folder("outline-close-running");
    let file = dir.join("a.rs");
    std::fs::write(&file, "fn one() {}\n").unwrap();
    let mut winc = Winc::new(None, None);
    let a = winc.add_detached(app_with(&file, false));
    let b = winc.add_detached(app_with(&file, false));
    winc.app(a).show_outline();
    winc.app(b).show_outline();
    winc.symbols_tick_for_test();
    winc.app(a).request_close_window();
    winc.settle_for_test();
    winc.set_active(b);
    winc.symbols_for_test();
    assert_eq!(winc.app(b).symbols.index.as_ref().unwrap()[0].name, "one");
    assert!(!winc.keys().contains(&a));
    let _ = std::fs::remove_dir_all(dir);
}
