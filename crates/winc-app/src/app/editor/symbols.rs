//! 转到文件中的符号（`⇧⌘O`，或在快速打开中输入 `@`）。
//!
//! 在快速打开的卡片中列出当前文件的函数、类型、标题等符号，输入内容可筛选。上下移动选中项时，
//! 编辑区预览滚动到该符号；回车跳转到该符号，`Esc` 恢复打开前的滚动位置。

use crate::symbol_jobs::Key;
use winc_editor::DocumentId;
use winc_editor::symbols::{Symbol, SymbolIndex};
use winc_i18n::t;

use super::*;

/// 当前文档的符号列表，进入 `@` 模式时提取一次。
pub struct SymbolList {
    /// 符号所属的文档。
    id: DocumentId,
    /// 提取到的符号，按在文件中出现的顺序排列。
    pub items: Arc<SymbolIndex>,
    /// 文档是否为大文件（全文没有读入内存）；为真时不提取符号，`items` 为空。
    pub too_large: bool,
    /// 打开列表时编辑区的滚动位置：首行（行，从 0 起）和横向偏移（逻辑像素）。
    ///
    /// 未选择符号就离开时据此恢复。
    scroll: (f32, f32),
}

/// 当前活动窗口共享一份索引；后台窗口释放引用并暂停显示。
#[derive(Default)]
pub(crate) struct SymbolState {
    pub key: Option<Key>,
    pub index: Option<Arc<SymbolIndex>>,
    pub due: Option<Instant>,
    pub waiting: bool,
    pub paused: bool,
    pub error: bool,
    pub scroll: f32,
    pub collapsed: HashSet<usize>,
    pub visible: Vec<usize>,
    pub hits: Vec<(Rect, (usize, bool))>,
}

impl SymbolState {
    pub fn rebuild_rows(&mut self) {
        self.hits.clear();
        self.visible.clear();
        let Some(index) = &self.index else {
            return;
        };
        let mut hidden_depth = None;
        for (i, symbol) in index.iter().enumerate() {
            if hidden_depth.is_some_and(|depth| symbol.depth > depth) {
                continue;
            }
            hidden_depth = self.collapsed.contains(&i).then_some(symbol.depth);
            self.visible.push(i);
        }
    }
}

impl App {
    /// 当前显示文本的索引身份；非文本标签不生成大纲。
    pub(crate) fn symbol_key(&self) -> Option<Key> {
        if !self.active_shows_text() {
            return None;
        }
        let doc = self.editor.active()?;
        Some(Key {
            doc: doc.id,
            revision: doc.revision,
            language: doc.highlighter.language().id,
        })
    }

    /// 大纲和符号选择器共用一个消费者身份。
    pub(crate) fn wants_symbols(&self) -> bool {
        (self.panel_visible && self.sidebar_view == SidebarView::Outline)
            || (self.picker_open && self.picker_lists_symbols())
    }

    /// 展示大纲，沿用编辑区键盘焦点。
    pub(in crate::app) fn show_outline(&mut self) {
        if self.symbols.error {
            self.symbols.key = None;
        }
        self.set_sidebar_view(SidebarView::Outline);
        if !self.panel_visible {
            self.set_sidebar(true);
        }
        self.focus = Focus::Editor;
        self.request_redraw();
    }

    /// 大纲行点击；过期结果不能用于跳转。
    pub(in crate::app) fn click_outline(&mut self, item: usize, fold: bool) {
        if self.symbols.key != self.symbol_key() || self.symbols.waiting {
            return;
        }
        if fold {
            if !self.symbols.collapsed.remove(&item) {
                self.symbols.collapsed.insert(item);
            }
            self.symbols.rebuild_rows();
        } else if let Some(symbol) = self.symbols.index.as_ref().and_then(|s| s.get(item)) {
            let (line, col) = (symbol.line, symbol.col);
            self.jump_to(line, col, 0);
            self.focus = Focus::Editor;
        }
        self.request_redraw();
    }
    /// 打开符号列表（`⇧⌘O`）：在快速打开的卡片中列出当前文档的符号，不需要先打开目录。
    ///
    /// 没有打开的文档，或当前标签不显示文本时，在状态栏提示并返回。
    pub(in crate::app) fn open_symbols(&mut self) {
        if self.editor.active.is_none() || !self.active_shows_text() {
            self.flash(t!("editor-no-open-file"));
            return;
        }
        self.finish_name_input();
        self.picker_places = None;
        self.picker_branch_rows = None;
        self.picker_location_rows = None;
        self.picker_open = true;
        self.picker_doc = self.editor.active().map(|d| d.id);
        self.picker_input.set_text("@");
        self.picker_input.set_caret(1, false);
        self.focus = Focus::Picker;
        self.picker_refresh();
        self.relayout();
        self.reset_blink();
        self.request_redraw();
    }

    /// 是否正在列出符号，即输入框以 `@` 开头且不在「位置」列表中。
    pub(crate) fn picker_lists_symbols(&self) -> bool {
        self.picker_places.is_none()
            && !self.picker_lists_branches()
            && !self.picker_lists_locations()
            && self.picker_input.text().starts_with('@')
    }

    /// 按当前查询刷新符号列表。
    ///
    /// 查询为空时保持符号在文件中的顺序，并选中光标所在的符号；
    /// 否则按名字的匹配度（见 [`input::palette::rate`]）排序，匹配度相同时保持文件中的顺序。
    pub(crate) fn symbols_refresh(&mut self) {
        let Some(doc) = self.editor.active() else {
            self.picker_symbols = None;
            self.picker_hits.clear();
            return;
        };
        {
            let items = self.symbols.index.clone().unwrap_or_default();
            let scroll = self
                .picker_symbols
                .as_ref()
                .filter(|l| l.id == doc.id)
                .map_or((doc.scroll_line, doc.scroll_x), |l| l.scroll);
            self.picker_symbols = Some(SymbolList {
                id: doc.id,
                items,
                too_large: doc
                    .text()
                    .rope()
                    .is_none_or(|r| r.len_bytes() > winc_editor::symbols::INDEX_INPUT_BYTES),
                scroll,
            });
        }
        let Some(list) = &self.picker_symbols else {
            return;
        };

        let needle = input::palette::needle(self.picker_input.text().trim_start_matches('@'));
        let caret_line = self
            .editor
            .active()
            .map(|d| d.text().pos_to_line_col(d.selection.head).0);
        let mut scored: Vec<(u32, usize)> = list
            .items
            .iter()
            .enumerate()
            .filter_map(|(i, s)| {
                let score = if needle.is_empty() {
                    0
                } else {
                    input::palette::rate(&needle, &s.name)?
                };
                Some((score, i))
            })
            .collect();
        scored.sort();
        // 查询为空时选中光标所在的符号，即起始行不晚于光标行的最后一个符号，
        // 使列表打开时就定位在当前位置。
        if needle.is_empty()
            && let Some(caret) = caret_line
        {
            self.picker_selected = list
                .items
                .iter()
                .rposition(|s| s.line <= caret)
                .unwrap_or(0);
        }
        self.picker_hits = scored.into_iter().map(|(_, i)| i).collect();
        self.symbols_preview();
    }

    /// 将编辑区滚动到选中的符号并预览，光标位置不变；该行已经可见时不滚动。
    pub(in crate::app) fn symbols_preview(&mut self) {
        let Some(line) = self.selected_symbol().map(|s| s.line) else {
            return;
        };
        self.scroll_to_line(line, 1.0 / 3.0, true);
        self.request_redraw();
    }

    fn selected_symbol(&self) -> Option<&Symbol> {
        if self.symbols.key != self.symbol_key() || self.symbols.waiting {
            return None;
        }
        let list = self.picker_symbols.as_ref()?;
        list.items.get(*self.picker_hits.get(self.picker_selected)?)
    }

    /// 跳转到选中的符号并选中其名字，同时关闭面板。
    pub(in crate::app) fn symbols_accept(&mut self) {
        let Some(symbol) = self.selected_symbol().cloned() else {
            return;
        };
        // 先清空 `picker_symbols`，关闭面板时 `drop_symbols` 就不会再恢复原来的滚动位置。
        self.picker_symbols = None;
        self.close_picker();
        self.jump_to(symbol.line, symbol.col, symbol.name_chars);
    }

    /// 离开 `@` 模式（关闭面板或删除 `@`）时清理符号列表。
    ///
    /// 未跳转到符号时，编辑区恢复到打开列表时的滚动位置。
    pub(in crate::app) fn drop_symbols(&mut self) {
        let Some(list) = self.picker_symbols.take() else {
            return;
        };
        if let Some(doc) = self.editor.document_mut(list.id) {
            (doc.scroll_line, doc.scroll_x) = list.scroll;
        }
    }
}
