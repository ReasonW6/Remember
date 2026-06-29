# Remember

Remember 是一个轻量级 Windows 桌面自动化应用，使用 Tauri、Rust、React 和
TypeScript 构建。它把本地前台操作记录为 `.remember.json` 流程文件，并在基本
安全检查通过后重放这些步骤。

English version: [README_en.md](README_en.md)

## 当前范围

Remember 目前支持完整 MVP 使用流程：

- 记录鼠标单击、双击、拖拽、滚轮、键盘文本、普通控制键、快捷键、等待时间和活动窗口元数据。
- 在工作台里查看录制步骤。
- 编辑延迟、点击坐标、输入文本、快捷键组合和目标窗口确认状态。
- 删除步骤，并插入等待、文本、快捷键和普通按键步骤。
- 保存和加载本地 `.remember.json` 流程，重启后仍可复用。
- 按速度和循环次数重放等待、点击、拖拽、文本、普通按键、快捷键和滚轮步骤。
- 通过界面按钮或全局急停热键 `Ctrl + Alt + S` 停止回放。
- 当目标窗口检查失败时，在运行日志和目标窗口区域显示安全停止原因。

Remember 以 Windows 和本地使用为优先。当前不包含云同步、OCR、图像识别、AI 规划、
插件、远程执行或跨平台支持。

## 安装或运行

成功构建后，可以使用 Windows 安装包：

```powershell
src-tauri\target\release\bundle\nsis\Remember_0.1.0_x64-setup.exe
```

开发运行：

```powershell
node --version  # 需要 Node.js 22.12 或更高版本
npm install
npm run tauri dev
```

开发服务器使用 `127.0.0.1:1450`。

## 日常使用流程

1. 打开 Remember，默认先显示紧凑控制窗。
2. 选择已保存流程，或从当前默认流程开始。
3. 点击 `录制`。
4. 确认录制安全提示。
5. 执行希望 Remember 捕获的安全本地操作。
6. 点击 `停止`。
7. 工作台会打开并显示录制得到的步骤。
8. 按需检查和编辑步骤。
9. 点击 `保存流程` 或 `另存为`。
10. 点击 `运行` 或 `重放` 开始回放。
11. 回放期间可随时按 `Ctrl + Alt + S` 急停。

## 支持的输入

Remember 会记录普通文本、鼠标操作、修饰键快捷键和常见控制键。比如 `Ctrl + F10`
会作为目标应用的快捷键步骤保存；单独的 `F10` 不会被记录，也不是启动回放的全局快捷键。

工作台是桌面编辑界面，最低窗口宽度为 1080 px。屏幕较小时，建议使用 Windows 显示缩放
或切换到更宽的显示器后再编辑密集步骤列表。

## 安全说明

- 不要录制密码、验证码、私密消息、支付信息或其他敏感字段。
- 输入类回放步骤需要目标窗口元数据。目标缺失或明显不一致时，Remember 会安全停止，而不是继续点击或输入。
- 无限循环回放必须显式确认，并且必须能通过界面或急停热键停止。
- Remember 面向前台桌面自动化，不是隐藏后台自动化运行器。

## 流程文件

流程会保存为应用本地的 `.remember.json` 文件。v1 数据结构保持小而明确：

```json
{
  "version": 1,
  "name": "daily-report",
  "displayName": "Daily Report",
  "targetWindow": {
    "title": "Report - Notepad",
    "process": "notepad.exe",
    "size": "1024 x 768",
    "matched": true
  },
  "steps": [
    { "type": "click", "id": 1, "action": "左键单击", "target": "(120, 240) [屏幕绝对]", "x": 120, "y": 240, "delayMs": 200, "note": "open menu" },
    { "type": "type", "id": 2, "action": "文本输入", "text": "Daily Report", "delayMs": 300, "note": "title" },
    { "type": "hotkey", "id": 3, "action": "快捷键", "keys": ["Ctrl", "S"], "delayMs": 100, "note": "save" },
    { "type": "wait", "id": 4, "action": "等待", "durationMs": 500, "delayMs": 500, "note": "pause" }
  ]
}
```

存储校验会拒绝格式错误、版本不支持、步骤 ID 重复、流程名缺失、空按键/空快捷键、
高风险全局快捷键、明显敏感的步骤文本或步骤元数据、超长步骤时长，以及 `durationMs`
和 `delayMs` 不一致的等待步骤。损坏的流程文件仍会显示在流程列表里，方便定位问题，
而不是被静默隐藏。

## 构建和验证

常用检查：

```powershell
npm test
npm run build
cargo fmt --manifest-path src-tauri\Cargo.toml --check
cargo check --manifest-path src-tauri\Cargo.toml
cargo test --manifest-path src-tauri\Cargo.toml
```

构建 Windows 安装包：

```powershell
npm run tauri build
```

输出路径：

```text
src-tauri\target\release\bundle\nsis\Remember_0.1.0_x64-setup.exe
```

Release 可执行文件路径：

```text
src-tauri\target\release\remember.exe
```

发布前必须使用有效 Authenticode 签名。`npm run verify:release-signature`
会检查 release 可执行文件和 NSIS 安装包；如果文件缺失、未签名或证书不受信任，检查会失败。
该命令使用 PowerShell 7 (`pwsh`) 运行。真正发布前仍需要可用的 Windows 代码签名证书。

发布前签名检查：

```powershell
npm run verify:release-signature
```

## 已知后续打磨

- 应用图标位于 `src-tauri/icons/icon.ico`，可编辑源文件为 `src-tauri/icons/remember-icon.svg`。
- 托盘行为目前不属于 MVP；紧凑置顶控制窗和全局急停热键已经能保持安全控制可见。
- 高级目标窗口匹配控制不属于当前 MVP。
