//! 全进程串行的符号任务；线程按需启动、空闲退出，失败绝不在调用线程扫描。

use std::io;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use ropey::Rope;
use winc_editor::DocumentId;
use winc_editor::symbols::{SymbolIndex, scan_symbols};
use winc_syntax::Language;

use crate::app::UserEvent;
use crate::winc::{Envelope, Recipient, WindowKey};
use winit::event_loop::EventLoopProxy;

/// 文档和解释该文本所用的语言版本。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Key {
    pub doc: DocumentId,
    pub revision: u64,
    pub language: &'static str,
}

struct Job {
    token: u64,
    window: WindowKey,
    key: Key,
    text: Rope,
    language: &'static Language,
    cancel: Arc<AtomicBool>,
}

type Launch = fn(Box<dyn FnOnce() + Send>) -> io::Result<()>;

fn launch(task: Box<dyn FnOnce() + Send>) -> io::Result<()> {
    thread::Builder::new()
        .name("winc-symbols".into())
        .spawn(task)
        .map(|_| ())
}

/// 全局完成回执；窗口关闭不能阻止槽位回收。
pub(crate) struct Completed {
    pub token: u64,
    pub window: WindowKey,
    pub key: Key,
    pub result: Result<Option<SymbolIndex>, ()>,
}

/// 唯一运行槽位和可复用的后台线程。
pub(crate) struct SymbolJobs {
    launch: Launch,
    worker: Option<Sender<Job>>,
    completed: (Sender<Completed>, Receiver<Completed>),
    running: Option<(u64, WindowKey, Key, Arc<AtomicBool>)>,
    next: u64,
}

impl Default for SymbolJobs {
    fn default() -> Self {
        Self::with_launcher(launch)
    }
}

impl SymbolJobs {
    fn with_launcher(launch: Launch) -> Self {
        Self {
            launch,
            worker: None,
            completed: mpsc::channel(),
            running: None,
            next: 0,
        }
    }

    pub fn busy(&self) -> bool {
        self.running.is_some()
    }

    pub fn cancel_unless(&self, desired: Option<(WindowKey, Key)>) {
        if let Some((_, window, key, cancel)) = &self.running
            && desired != Some((*window, *key))
        {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    pub fn poll(&mut self) -> Option<Completed> {
        let done = self.completed.1.try_recv().ok()?;
        if self.running.as_ref().is_some_and(|r| r.0 == done.token) {
            self.running = None;
        }
        Some(done)
    }

    pub fn start(
        &mut self,
        window: WindowKey,
        key: Key,
        text: Rope,
        language: &'static Language,
        proxy: Option<EventLoopProxy<Envelope>>,
    ) -> io::Result<()> {
        if self.busy() {
            return Err(io::Error::other("symbol scanner busy"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.next += 1;
        let token = self.next;
        let mut job = Job {
            token,
            window,
            key,
            text,
            language,
            cancel: cancel.clone(),
        };
        if let Some(worker) = &self.worker {
            match worker.send(job) {
                Ok(()) => {
                    self.running = Some((token, window, key, cancel));
                    return Ok(());
                }
                Err(error) => job = error.0,
            }
        }
        let (sender, receiver) = mpsc::channel::<Job>();
        let results = self.completed.0.clone();
        (self.launch)(Box::new(move || {
            while let Ok(job) = receiver.recv_timeout(Duration::from_secs(10)) {
                let Job {
                    token,
                    window,
                    key,
                    text,
                    language,
                    cancel,
                } = job;
                // 测试的 unwind 同样交回终态；release abort 时整个进程退出。
                let result = std::panic::catch_unwind(|| scan_symbols(&text, language, &cancel))
                    .map_err(|_| ());
                drop(text);
                let _ = results.send(Completed {
                    token,
                    window,
                    key,
                    result,
                });
                if let Some(proxy) = &proxy {
                    let _ = proxy.send_event(Envelope {
                        to: Recipient::Active,
                        event: UserEvent::SymbolsReady,
                    });
                }
            }
        }))?;
        sender
            .send(job)
            .map_err(|_| io::Error::other("symbol scanner stopped"))?;
        self.worker = Some(sender);
        self.running = Some((token, window, key, cancel));
        Ok(())
    }
}

impl Drop for SymbolJobs {
    fn drop(&mut self) {
        self.cancel_unless(None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_creation_failure_does_not_run_scanner_inline() {
        let mut windows = crate::winc::Winc::new(None, None);
        let (window, _) = windows.reserve();
        let mut jobs =
            SymbolJobs::with_launcher(|_| Err(io::Error::other("injected spawn failure")));
        let key = Key {
            doc: DocumentId(42),
            revision: 0,
            language: "rust",
        };
        assert!(
            jobs.start(
                window,
                key,
                Rope::from_str("fn one() {}"),
                winc_syntax::detect("x.rs"),
                None
            )
            .is_err()
        );
        assert!(!jobs.busy());
        assert!(jobs.poll().is_none());
        assert!(jobs.worker.is_none());
    }

    #[test]
    fn canceled_job_releases_global_slot_and_next_job_runs() {
        let mut windows = crate::winc::Winc::new(None, None);
        let (window, _) = windows.reserve();
        let mut jobs = SymbolJobs::default();
        let key = Key {
            doc: DocumentId(42),
            revision: 0,
            language: "rust",
        };
        let text = Rope::from_str("fn old() {}\n");
        jobs.start(window, key, text, winc_syntax::detect("x.rs"), None)
            .unwrap();
        jobs.cancel_unless(None);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        loop {
            if jobs.poll().is_some() {
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(2));
        }
        assert!(!jobs.busy());
        let next = Key { revision: 1, ..key };
        jobs.start(
            window,
            next,
            Rope::from_str("fn fresh() {}\n"),
            winc_syntax::detect("x.rs"),
            None,
        )
        .unwrap();
        loop {
            if let Some(done) = jobs.poll() {
                assert_eq!(done.key, next);
                assert_eq!(done.result.unwrap().unwrap()[0].name, "fresh");
                break;
            }
            assert!(std::time::Instant::now() < deadline);
            thread::sleep(Duration::from_millis(2));
        }
        assert!(!jobs.busy());
    }
}
