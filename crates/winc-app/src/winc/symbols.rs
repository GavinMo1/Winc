//! 活动窗口的符号需求调度；完成回收独立于发起窗口的生命周期。

use std::sync::Arc;
use std::time::{Duration, Instant};
use winc_editor::symbols::{INDEX_INPUT_BYTES, ScanLimit, SymbolIndex};

use super::*;

impl Winc {
    pub(super) fn sync_symbols(&mut self) -> Option<Instant> {
        let now = Instant::now();
        let active = self.active_key();
        let desired = self
            .windows
            .iter()
            .find(|s| Some(s.key) == active)
            .filter(|s| s.app.wants_symbols())
            .and_then(|s| s.app.symbol_key().map(|key| (s.key, key)));
        self.symbols.cancel_unless(desired);
        while let Some(done) = self.symbols.poll() {
            if desired != Some((done.window, done.key)) {
                continue;
            }
            let Some(slot) = self.windows.iter_mut().find(|s| s.key == done.window) else {
                continue;
            };
            let state = &mut slot.app.symbols;
            if state.key != Some(done.key) {
                continue;
            }
            match done.result {
                Ok(Some(index)) => {
                    state.index = Some(Arc::new(index));
                    state.waiting = false;
                    state.due = None;
                    state.error = false;
                    state.collapsed.clear();
                    state.rebuild_rows();
                }
                Ok(None) => state.due = Some(now),
                Err(()) => {
                    state.error = true;
                    state.waiting = false;
                    state.due = None;
                }
            }
            if slot.app.picker_open && slot.app.picker_lists_symbols() {
                slot.app.symbols_refresh();
            }
            slot.app.request_redraw();
        }
        for slot in &mut self.windows {
            let current = desired
                .filter(|(window, _)| *window == slot.key)
                .map(|(_, key)| key);
            let app = &mut slot.app;
            if current.is_none() {
                if app.symbols.key.is_some() || !app.symbols.paused {
                    app.symbols = Default::default();
                    app.symbols.paused = true;
                    if let Some(list) = &mut app.picker_symbols {
                        list.items = Arc::default();
                        app.picker_hits.clear();
                    }
                    app.request_redraw();
                }
                continue;
            }
            let key = current.unwrap();
            if app.symbols.key != Some(key) {
                let editing = app.symbols.key.is_some_and(|old| old.doc == key.doc);
                app.symbols.key = Some(key);
                app.symbols.index = None;
                app.symbols.visible.clear();
                app.symbols.hits.clear();
                app.symbols.collapsed.clear();
                app.symbols.paused = false;
                app.symbols.waiting = true;
                app.symbols.error = false;
                app.symbols.due = Some(if editing {
                    now + Duration::from_millis(300)
                } else {
                    now
                });
                app.symbols.scroll = 0.0;
                if let Some(list) = &mut app.picker_symbols {
                    list.items = Arc::default();
                    app.picker_hits.clear();
                }
                app.request_redraw();
            }
            if app.symbols.due.is_none_or(|due| due > now) {
                continue;
            }
            let Some(doc) = app.editor.active() else {
                continue;
            };
            let rope = doc
                .text()
                .rope()
                .filter(|r| r.len_bytes() <= INDEX_INPUT_BYTES);
            let Some(rope) = rope else {
                app.symbols.index = Some(Arc::new(SymbolIndex {
                    items: Vec::new(),
                    limit: Some(ScanLimit::Input),
                }));
                app.symbols.waiting = false;
                app.symbols.due = None;
                app.symbols.rebuild_rows();
                if app.picker_open && app.picker_lists_symbols() {
                    app.symbols_refresh();
                }
                app.request_redraw();
                continue;
            };
            if self.symbols.busy() {
                continue;
            }
            // 创建快照前已在 UI 线程检查实际长度，队列中不保存过时快照。
            let outcome = self.symbols.start(
                slot.key,
                key,
                rope.clone(),
                doc.highlighter.language(),
                self.proxy.clone(),
            );
            app.symbols.due = None;
            if outcome.is_err() {
                app.symbols.error = true;
                app.symbols.waiting = false;
                app.request_redraw();
            }
        }
        self.windows
            .iter()
            .filter_map(|s| s.app.symbols.due)
            .filter(|due| *due > now)
            .min()
    }
}
