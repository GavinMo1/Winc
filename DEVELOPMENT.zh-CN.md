# Windows 开发环境与双电脑协作

本说明面向 Windows x64 开发。Fork 为 `GavinMo1/Winc`，原作者仓库为
`DEEIX-AI/Winc`，原作者的开发分支为 `dev`。我们在 `development` 分支开发。

## 环境需求

| 工具 | 要求与用途 |
| --- | --- |
| Git | 获取代码、同步两台电脑和原作者修改；已准备，不再介绍安装 |
| Windows 原生链接工具 | 默认 MSVC 路径需 C++ Build Tools 和 Windows SDK；也可使用下方 GNU 方案 |
| Scoop | 安装和更新 rustup；以下假定已安装 |
| rustup | 管理 Rust 工具链，由 Scoop 安装 |
| rustc、Cargo | Rust 编译器和构建工具，由 rustup 安装，不需要单独安装 |
| rustfmt、Clippy | 格式化和静态检查，由项目工具链配置安装 |

项目使用 Rust 原生界面，无需先安装前端依赖、数据库或 Docker。Node.js、Python 已有，
本说明不再介绍安装；普通 Rust 构建不需要它们。

## 通过 Scoop 安装 Rust

在每台电脑的普通 PowerShell 中执行：

```powershell
scoop update
scoop install rustup
```

`rustup` 位于 Scoop 的 `main` bucket，默认使用 MSVC 工具链。不要再另外安装 Scoop 的
独立 `rust` 包，以免两套工具的 PATH 混用。Scoop 管理 rustup 本身，rustup 管理编译器版本。

重开 PowerShell，进入仓库目录后执行：

```powershell
cd D:\Winc
rustup set default-host x86_64-pc-windows-msvc
rustup toolchain install
rustup show
rustc --version
cargo --version
cargo fmt --version
cargo clippy --version
```

仓库根目录的 `rust-toolchain.toml` 指定 Rust **1.99.1**、`rustfmt` 和 `clippy`。
`rustup toolchain install` 在这里读取该配置。若本机已有 GNU 工具链，执行
`rustup toolchain install 1.99.1-x86_64-pc-windows-msvc --profile minimal --component rustfmt --component clippy`，
并确认 `rustup show` 显示当前项目使用 MSVC 工具链。

首次安装和构建需要访问 Rust 工具链下载站点及 crates.io。以后同步上游时，如工具链配置
发生变化，再运行 `rustup toolchain install`。不要为绕过下载错误自行修改项目锁定版本。

更新 rustup 管理器用 `scoop update rustup`；无需把项目编译器改成最新 stable。

## 验证已有的 Visual Studio 工具

### 不安装 Visual Studio 的本机开发选项

本仓库默认和发布 CI 使用 MSVC；本机没有 Build Tools 时也可以用独立的 Rust GNU 工具链开发，
不修改项目工具链文件。在 Windows x64 上通过 Scoop 安装 rustup 后执行：

```powershell
rustup toolchain install 1.99.1-x86_64-pc-windows-gnu --profile minimal --component rustfmt --component clippy
scoop install mingw
$env:RUSTUP_TOOLCHAIN = '1.99.1-x86_64-pc-windows-gnu'
$gnuTools = "$env:USERPROFILE\scoop\apps\mingw\current\bin"
$env:Path = "$gnuTools;$env:Path"
cargo check --workspace --all-targets --locked --target x86_64-pc-windows-gnu
cargo test --workspace --locked --target x86_64-pc-windows-gnu
cargo build --locked --target x86_64-pc-windows-gnu -p winc-app -p winc-pty
cargo run --locked --target x86_64-pc-windows-gnu
```

此处 `RUSTUP_HOME` 由 Scoop rustup 设置；若当前会话读不到，重开 PowerShell。
MinGW 提供 GCC、dlltool 和汇编器；rustup 自带的链接工具不足以单独完成 windows-sys 的导入库生成。
安装后重开 PowerShell 即可使用其 PATH，上方赋值也可刷新当前会话。环境变量只影响当前会话，关闭后恢复默认工具链选择。
切回已具备 C++ 工具的 MSVC 环境，可执行 `Remove-Item Env:RUSTUP_TOOLCHAIN` 并重开终端。

GNU 验证不代替发布流程的 MSVC 验证，尤其本项目构建脚本只在 MSVC 下嵌入 Windows 图标资源。
制作正式安装包仍按发布配置准备工具。

### MSVC 环境检查

安装了 Visual Studio 不一定代表安装了 C++ 工作负载。可用下面的只读命令检查：

```powershell
$vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
& $vswhere -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
```

如果没有输出，在 Visual Studio Installer 中检查“使用 C++ 的桌面开发”，并确认 MSVC
x64/x86 工具和 Windows 10/11 SDK 已安装。普通 PowerShell 中找不到 `cl.exe` 不代表未安装；
必要时可用 Visual Studio 的 Developer PowerShell 验证构建。

## 首次构建与日常检查

在仓库根目录运行：

```powershell
cargo check --workspace --locked
cargo build --locked -p winc-app -p winc-pty
cargo run --locked
```

`winc-pty` 是内置终端使用的辅助程序，因此首次准备环境时一起构建。评估启动和滚动性能时
使用 release 构建：

```powershell
cargo build --release --locked -p winc-app -p winc-pty
cargo run --release --locked
```

提交前检查：

```powershell
cargo fmt --all --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo xtask test --all
$env:RUSTDOCFLAGS = '-D warnings'
cargo doc --workspace --no-deps --locked
Remove-Item Env:RUSTDOCFLAGS
```

日常只测试当前改动涉及的 crate 可以使用 `cargo xtask test`。
推荐编辑器扩展为 rust-analyzer；CMake、Ninja、Clang 当前无需额外准备。

## 家里电脑接着开发

第一次将开发分支推送到自己的 Fork（在已有仓库的电脑执行）：

```powershell
git push -u origin development
```

推送只包含已提交的修改，因此先将需要同步的文件提交。家里电脑准备好上述环境后：

```powershell
git clone --branch development https://github.com/GavinMo1/Winc.git D:\Winc
cd D:\Winc
rustup toolchain install
cargo build --locked -p winc-app -p winc-pty
```

路径可按实际情况替换。两台电脑开始工作前执行 `git pull --ff-only`；结束工作后提交并
`git push`。切换电脑前确认需要带走的修改已推送。`target` 构建产物和本机 Rust 安装不用同步。
如果两台电脑都产生了未同步提交，先检查差异再合并；不要通过强制推送覆盖另一台的工作。

## 同步原作者修改

每个本地仓库只需添加一次上游远程：

```powershell
git remote add upstream https://github.com/DEEIX-AI/Winc.git
git remote -v
```

如果已经存在 `upstream`，检查地址即可，不要重复添加。同步前先提交当前修改，确保工作区干净：

```powershell
git switch development
git pull --ff-only
git fetch upstream
git merge upstream/dev
```

如果发生冲突，逐个处理并提交合并结果，然后运行上面的检查命令。检查通过后：

```powershell
git push origin development
```

此流程通过 merge 保留原作者和自己的历史，适合两台电脑共用一个开发分支。

## 仅制作安装包时需要的工具

普通开发和运行不需要安装包工具。Windows 发布 CI 当前使用 **NSIS 3.11** 和
**WiX Toolset 3.14.1**，另有 Node.js 发布脚本。需要制作安装包时再参考
[发布说明](release/README.md) 和 `.github/workflows/release-windows.yml`。
Scoop 包可能已升级，不能假定其最新 WiX 与项目使用的 WiX 3 命令兼容，因此这里不提供
未经验证的 Scoop 打包工具安装命令。
