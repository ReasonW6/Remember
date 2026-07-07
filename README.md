# Remember

[English](README_en.md)

Remember 是一个面向 Windows 的轻量级录制回放工具，使用方式接近 TinyTask：按下快捷键开始录制键盘和鼠标操作，再按一次停止，之后可以把录制结果保存为本地 `.remember.json` 文件并重复回放。

Remember 是原创实现，不包含 TinyTask 的代码、图标、名称、二进制文件或其他资产。

## 主要功能

- 录制键盘和鼠标操作。
- 回放当前录制或从录制文件列表中选择一个文件回放。
- 支持循环次数和回放速度设置。
- 支持自定义快捷键，快捷键可以是单键，也可以是组合键。
- 录制和停止快捷键可以相同，默认使用 `F8` 作为录制/停止切换。
- 播放开始、播放结束、录制开始、录制结束会播放提示音。
- 使用自定义标题栏和应用内中文界面。

## 默认快捷键

- `F8`：开始录制；录制中再次按下会停止录制；回放中按下会停止回放。
- `F12`：开始回放。

播放快捷键不能和录制或停止快捷键相同。录制和停止快捷键可以相同。

## 录制文件

录制文件会保存为 `.remember.json`。应用内的“录制文件”列表会显示本地保存的录制文件，并支持选择、回放和删除。

保存到录制库的文件位于系统应用数据目录下的 `recordings` 文件夹。例如：

```text
%APPDATA%\com.remember.desktop\recordings
```

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

## 打包

创建 release 构建：

```powershell
npm run tauri build
```

构建产物位于：

```text
src-tauri\target\release
```

## 当前限制

- 目前只面向 Windows。
- 不是 AI 自动化工具，也不做图像识别。
- 回放真实键盘和鼠标输入，运行时焦点和目标窗口状态会影响结果。
- 以普通权限运行时，可能无法控制管理员权限窗口。
