//! 符号提取的测试：各语言的声明与方法、注释和字符串中的内容不误判、标题与小节的层次。

use super::*;

#[test]
fn bounded_scan_tracks_parent_identity_and_markdown_fences() {
    let cancel = AtomicBool::new(false);
    let text = Rope::from_str(
        "# Same\n## Child\n# Same\n## Other\n````md\n```\n# Hidden\n````\n   # Visible\n# C#\n",
    );
    let index = scan_symbols(&text, winc_syntax::detect("x.md"), &cancel).unwrap();
    assert_eq!(
        index.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["Same", "Child", "Same", "Other", "Visible", "C#"]
    );
    assert_eq!(index[1].parent, Some(0));
    assert_eq!(index[3].parent, Some(2));
    assert_eq!(index[4].parent, None);
    assert_eq!(index.limit, None);
}

#[test]
fn bounded_scan_rejects_large_ropes_and_stops_before_overlong_lines() {
    let cancel = AtomicBool::new(false);
    let mut text = Rope::from_str("fn first() {}\n");
    text.append(Rope::from_str(&"x".repeat(INDEX_LINE_BYTES + 1)));
    let index = scan_symbols(&text, winc_syntax::detect("x.rs"), &cancel).unwrap();
    assert_eq!(index.limit, Some(ScanLimit::Line));
    assert_eq!(index[0].name, "first");
    let large = Rope::from_str(&"x".repeat(INDEX_INPUT_BYTES + 1));
    assert_eq!(
        scan_symbols(&large, winc_syntax::detect("x.rs"), &cancel)
            .unwrap()
            .limit,
        Some(ScanLimit::Input)
    );
    cancel.store(true, Ordering::Relaxed);
    assert!(scan_symbols(&text, winc_syntax::detect("x.rs"), &cancel).is_none());
}

#[test]
fn bounded_scan_truncates_display_names_at_utf8_boundaries_and_caps_results() {
    let cancel = AtomicBool::new(false);
    let name = "名".repeat(200);
    let text = Rope::from_str(&format!("fn {name}() {{}}\n"));
    let index = scan_symbols(&text, winc_syntax::detect("x.rs"), &cancel).unwrap();
    assert_eq!(index[0].name_chars, 200);
    assert!(index[0].name.len() <= INDEX_NAME_BYTES);
    assert!(name.starts_with(&index[0].name));
    let dense: String = (0..15_000)
        .map(|i| format!("fn item_{i}() {{}}\n"))
        .collect();
    let index = scan_symbols(
        &Rope::from_str(&dense),
        winc_syntax::detect("x.rs"),
        &cancel,
    )
    .unwrap();
    assert!(index.len() <= INDEX_NODES);
    assert!(index.limit.is_some());
}

fn outline(language: &str, text: &str) -> Vec<(String, &'static str, Option<String>)> {
    let language = winc_syntax::detect(&format!("x.{language}"));
    symbols(&Rope::from_str(text), language)
        .into_iter()
        .map(|s| (s.name, s.kind.label(), s.container))
        .collect()
}

fn names(language: &str, text: &str) -> Vec<String> {
    outline(language, text)
        .into_iter()
        .map(|(n, ..)| n)
        .collect()
}

#[test]
fn rust_items_and_impls() {
    // 样本的 `//!` 注释和块注释中各含有一个 `fn`，二者都不应被识别为符号。
    let text = "\
//! fn not_this
use std::fmt;

pub const LIMIT: usize = 3;

pub(crate) struct Server {
    port: u16,
}

impl fmt::Display for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let inner = 1;
        Ok(())
    }
}

/* fn hidden() {
} */
pub async fn run() {}
macro_rules! log { () => {} }
";
    assert_eq!(
        outline("rs", text),
        [
            ("LIMIT".into(), "变量", None),
            ("Server".into(), "类型", None),
            ("fmt::Display for Server".into(), "模块", None),
            ("fmt".into(), "方法", Some("fmt::Display for Server".into())),
            ("run".into(), "函数", None),
            ("log".into(), "函数", None),
        ]
    );
}

#[test]
fn python_classes_methods_and_module_names() {
    // 样本的三引号字符串和 `#` 注释中各含有一个 `def`，二者都不应被识别为符号。
    let text = "\
\"\"\"
def not_this():
\"\"\"
VERSION = \"1\"

class Shop:
    tax = 0.1

    def total(self):
        count = 1
        def inner():
            pass

# def commented():
async def main():
    pass
";
    assert_eq!(
        outline("py", text),
        [
            ("VERSION".into(), "变量", None),
            ("Shop".into(), "类型", None),
            ("total".into(), "方法", Some("Shop".into())),
            ("inner".into(), "函数", Some("total".into())),
            ("main".into(), "函数", None),
        ]
    );
}

#[test]
fn javascript_functions_classes_and_methods() {
    let text = "\
export default class Cart {
  constructor(items) {
    this.items = items;
  }
  async total(tax) {
    if (tax) {
      return 1;
    }
  }
}
export const sum = (a, b) => a + b;
const label = 'x';
function render() {}
describe('cart', () => {
  it(\"adds\", function () {
  });
});
";
    assert_eq!(
        outline("js", text),
        [
            ("Cart".into(), "类型", None),
            ("constructor".into(), "方法", Some("Cart".into())),
            ("total".into(), "方法", Some("Cart".into())),
            ("sum".into(), "函数", None),
            ("label".into(), "变量", None),
            ("render".into(), "函数", None),
        ]
    );
}

#[test]
fn go_c_and_java() {
    let go = "type Server struct {}\n\nfunc (s *Server) Start() error {\n}\n\nfunc main() {\n}\n";
    assert_eq!(
        outline("go", go),
        [
            ("Server".into(), "类型", None),
            ("Start".into(), "方法", Some("Server".into())),
            ("main".into(), "函数", None),
        ]
    );

    let c = "#define MAX 3\nstatic int count(const char *s)\n{\n    return strlen(s);\n}\nint x = foo(3);\n";
    assert_eq!(names("c", c), ["MAX", "count"]);

    let java = "public class App {\n    public static void main(String[] args) {\n        while (true) {\n        }\n    }\n}\n";
    assert_eq!(
        outline("java", java),
        [
            ("App".into(), "类型", None),
            ("main".into(), "方法", Some("App".into())),
        ]
    );
}

#[test]
fn headings_sections_and_others() {
    let md = "# Title\n\n```sh\n# not a heading\n```\n## Install ##\n### Mac\n## Usage\n";
    assert_eq!(
        outline("md", md),
        [
            ("Title".into(), "标题", None),
            ("Install".into(), "标题", Some("Title".into())),
            ("Mac".into(), "标题", Some("Install".into())),
            ("Usage".into(), "标题", Some("Title".into())),
        ]
    );

    assert_eq!(
        names("toml", "[package]\nname = \"x\"\n[[bin]]\n"),
        ["package", "bin"]
    );
    assert_eq!(
        names("sh", "# deploy() {\ndeploy() {\n}\nfunction clean {\n}\n"),
        ["deploy", "clean"]
    );
    assert_eq!(
        names(
            "sql",
            "CREATE TABLE IF NOT EXISTS users (\n);\ncreate view v as select 1;\n"
        ),
        ["users", "v"]
    );
    assert!(names("json", "{\"a\": 1}").is_empty());
}
