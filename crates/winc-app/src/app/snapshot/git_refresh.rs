//! 日志刷新回归在 Windows 同样运行，不依赖 Unix 的 Git 快照模块。

use super::*;
use crate::app::git::log::LogTarget;

#[test]
fn refresh_button_click_loads_external_commit_in_narrow_log() {
    let dir = sample_folder("git-refresh-click");
    let git = |args: &[&str]| {
        std::process::Command::new("git")
            .arg("-C")
            .arg(&dir)
            .args([
                "-c",
                "user.name=test",
                "-c",
                "user.email=test@example.com",
                "-c",
                "commit.gpgsign=false",
            ])
            .args(args)
            .output()
            .unwrap()
    };
    assert!(
        git(&[
            "init",
            "-q",
            "-b",
            "a-very-long-development-branch-name-for-refresh-testing"
        ])
        .status
        .success()
    );
    assert!(git(&["add", "."]).status.success());
    assert!(git(&["commit", "-q", "-m", "before"]).status.success());
    let mut app = app_with(&dir, false);
    app.settings_toggle(Setting::GitModule);
    app.renderer =
        Some(Renderer::offscreen(760.0, 600.0, 1.5, app.palette.frame_bg, None, None).unwrap());
    app.run_action(crate::keymap::Action::GitShowLog);
    paint(&mut app);
    assert!(
        app.editor
            .active()
            .unwrap()
            .log
            .as_ref()
            .unwrap()
            .commits
            .iter()
            .any(|c| c.subject == "before")
    );
    assert!(
        git(&["commit", "-q", "--allow-empty", "-m", "after"])
            .status
            .success()
    );
    // 已打开页面，不触发 git_refresh 自动刷新；经绘制得到的按钮坐标点击。
    let (rect, _) = app
        .log
        .hits
        .iter()
        .find(|(_, target)| *target == LogTarget::Refresh)
        .unwrap();
    let point = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    assert_eq!(
        app.hover_at(point.0, point.1),
        Hover::Log(LogTarget::Refresh)
    );
    app.on_mouse_move(point.0, point.1);
    app.on_mouse_down(MouseButton::Left);
    app.on_mouse_up(MouseButton::Left);
    paint(&mut app);
    let view = app.editor.active().unwrap().log.as_ref().unwrap();
    assert_eq!(view.commits[0].subject, "after");
    assert_eq!(
        view.query.branch.as_deref(),
        Some("a-very-long-development-branch-name-for-refresh-testing")
    );
    drop(app);
    let _ = std::fs::remove_dir_all(dir);
}
