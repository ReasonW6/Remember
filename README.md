# Remember

[English](README_en.md)

Remember 是一个面向 Windows 的轻量级录制回放工具，使用方式接近 TinyTask：按下快捷键开始录制键盘和鼠标操作，再按一次停止；结果会自动保存到本地录制库，也可以另行导出为 `.remember.json` 文件并重复回放。

Remember 是原创实现，不包含 TinyTask 的代码、图标、名称、二进制文件或其他资产。

## 主要功能

- 录制键盘和鼠标操作。
- 回放当前录制或从录制文件列表中选择一个文件回放。
- 支持有限循环、无限循环和回放速度设置。
- 支持自定义快捷键；无修饰单键仅允许 `F1`–`F24`，字符、编辑和导航键必须与 `Ctrl`、`Alt`、`Shift` 或 `Win` 组合。
- 录制和停止快捷键可以相同，默认使用 `F8` 作为录制/停止切换；回放时播放键和停止键都可以终止回放。
- 播放开始、播放结束、录制开始、录制结束会播放提示音。
- 使用自定义标题栏和应用内中文界面。

## 默认快捷键

- `F8`：开始录制；录制中再次按下会停止录制；回放中作为独立停止键。
- `F12`：就绪时开始回放；回放中再次按下会停止回放。

播放快捷键不能和录制或停止快捷键相同。录制和停止快捷键可以相同。为避免劫持正常输入，无修饰快捷键只能使用 `F1`–`F24`。

回放期间可以按 `F8` 或 `F12` 停止。应用会先释放仍处于按下状态的按键和鼠标按钮；清理期间状态仍是回放中并显示“正在停止回放”，清理完成后才回到就绪。

## 录制文件

录制文件会保存为 `.remember.json`。每次停止录制时，应用都会自动把当前录制保存到本地录制库；“保存”按钮用于把当前录制另外导出到用户选择的位置。应用内的“录制文件”列表支持选择、回放、重命名和删除；按住 `Ctrl` 点击删除可以跳过确认。损坏或无法读取的录制文件仍会保留在列表中并显示错误，但不会被加载或回放。

录制库位于 `remember.exe` 同级的 `recordings` 文件夹：

```text
<软件所在目录>\recordings
```

把软件目录放在 D 盘时，录制文件也会保存在 D 盘。软件所在目录必须允许当前用户写入；不建议把便携版放在需要管理员权限才能写入的目录。旧版 `%APPDATA%\com.remember.desktop\recordings` 中的文件不会被自动移动或删除。

录制文件是未加密的 JSON，其中包含按键虚拟键码、扫描码、按下/释放时序以及鼠标位置。它可能反映密码、令牌或其他敏感输入。不要录制敏感信息；共享、备份或上传录制文件前请先检查内容，并及时删除不再需要的录制。

## 回放安全

- 循环次数可以是大于等于 1 的有限整数，也可以显式选择无限循环。
- 无限循环不会自行结束，必须使用播放键或停止键终止。
- Remember 有意不做目标窗口校验：回放会把真实输入发送给当时获得焦点的窗口，鼠标位置和窗口布局也会影响结果。
- 回放从他人获得或较早保存的录制前，应先确认焦点、目标窗口和录制来源可信。

## 环境要求

- Windows
- Node.js 22.12 或更高版本
- Rust stable
- Tauri 2 的 Windows 构建环境

## 开发运行

安装依赖：

```powershell
npm install
```

启动桌面应用开发模式：

```powershell
npm run tauri dev
```

开发模式会同时启动前端开发服务和 Tauri 应用。正式 release 可执行文件使用 Windows GUI 子系统，不会额外弹出控制台黑框。

## 测试与构建

运行前端测试：

```powershell
npm test
```

构建前端：

```powershell
npm run build
```

运行 Rust 测试：

```powershell
cargo test --manifest-path src-tauri\Cargo.toml
```

检查 Rust 编译：

```powershell
cargo check --manifest-path src-tauri\Cargo.toml
```

检查 Rust 格式：

```powershell
cargo fmt --manifest-path src-tauri\Cargo.toml -- --check
```

检查 Rust lint 和依赖安全：

```powershell
cargo clippy --manifest-path src-tauri\Cargo.toml --all-targets --all-features --locked -- -D warnings
npm audit
```

CI 还会使用 RustSec 检查 `src-tauri\Cargo.lock`。本机已安装 `cargo-audit` 时可以执行同一类检查：

```powershell
cargo audit --file src-tauri\Cargo.lock
```

## 打包

创建 release 构建：

```powershell
npm run tauri build
```

构建产物位于：

```text
src-tauri\target\release
```

仓库中的 Windows CI 会运行前端测试、npm 审计、Rust 测试、Clippy 和 RustSec 审计，构建便携版 `remember.exe`，并生成 SHA-256 校验文件。CI 产物明确是未签名的候选版本；SHA-256 只能检测文件是否变化，不能证明发布者身份。

面向公众发布前，必须使用真实、受信任的 Authenticode 证书对 `remember.exe` 签名并加时间戳，再验证签名状态：

```powershell
Get-AuthenticodeSignature .\remember.exe
```

不要把自签名证书或仅有 SHA-256 的文件描述为正式签名版本。

## 当前限制

- 目前只面向 Windows。
- 不是 AI 自动化工具，也不做图像识别。
- 回放真实键盘和鼠标输入，运行时焦点和目标窗口状态会影响结果。
- 不做目标窗口校验；这是当前便携工具的明确设计取舍。
- 以普通权限运行时，可能无法控制管理员权限窗口。

`docs/superpowers` 下的文档是早期设计和实施记录，可能保留历史快捷键或范围描述；当前用户行为以本 README、测试和源码为准。
