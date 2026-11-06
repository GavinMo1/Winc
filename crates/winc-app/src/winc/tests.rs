//! `winc` 的测试辅助：测试中没有事件循环，直接加入窗口、执行处理完一个事件后的那一步（见
//! `app/snapshot/windows.rs`）。

use super::*;

impl Winc {
    /// 测试用：只推进一轮，不等待正在运行的扫描。
    pub(crate) fn symbols_tick_for_test(&mut self) {
        self.sync_symbols();
    }
    /// 测试用：等待真实后台符号任务完成（不在 UI 上同步扫描）。
    pub(crate) fn symbols_for_test(&mut self) {
        let deadline = Instant::now() + std::time::Duration::from_secs(5);
        loop {
            let wake = self.sync_symbols();
            if !self.symbols.busy() && wake.is_none() {
                return;
            }
            assert!(Instant::now() < deadline, "符号任务未完成");
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// 测试用：取回一个窗口，以便 App 级交互测试复用进程级后台调度。
    pub(crate) fn take_app(&mut self, key: WindowKey) -> App {
        let at = self.windows.iter().position(|s| s.key == key).unwrap();
        self.windows.remove(at).app
    }
    /// 加入一个窗口，返回分到的编号（测试没有事件循环，用不到代理）。
    pub(crate) fn add_detached(&mut self, app: App) -> WindowKey {
        let (key, _) = self.reserve();
        self.add(key, app);
        key
    }

    /// 编号为 `key` 的窗口。
    ///
    /// # Panics
    ///
    /// 没有这个窗口时 panic。
    pub(crate) fn app(&mut self, key: WindowKey) -> &mut App {
        self.by_key(key).expect("没有这个窗口")
    }

    /// 测试用：按加入顺序返回所有窗口的编号。
    pub(crate) fn keys(&self) -> Vec<WindowKey> {
        self.windows.iter().map(|slot| slot.key).collect()
    }

    /// 测试用：把当前窗口设为 `key`。
    pub(crate) fn set_active(&mut self, key: WindowKey) {
        self.active = Some(key);
    }

    /// 测试用：跑一遍处理完事件后的那一步，返回窗口的请求和是否该结束事件循环。
    pub(crate) fn settle_for_test(&mut self) -> (Vec<Request>, bool) {
        self.settle()
    }

    /// 测试用：返回打开 `targets` 的窗口，语义见 `window_for`。
    pub(crate) fn window_for_test(
        &mut self,
        targets: &[Target],
    ) -> Result<WindowKey, Option<PathBuf>> {
        self.window_for(targets)
    }

    /// 测试用：记下其他窗口开着的目录。
    pub(crate) fn remember_windows_for_test(&mut self) {
        self.remember_windows();
    }
}
