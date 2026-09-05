## 本次改动

- `修复`：V2 标准 Windows 下拉框回放在每次 Windows 消息前后检查停止请求，取消后不再发送后续选择或通知消息。
- `修复`：当前录制使用后端提供的准确文件路径同步选择状态，删除同名录制后不会误选另一文件并播放旧内容。
- `安全`：更新 Browserslist 构建依赖，修复依赖审计报告的问题。
- `验证`：补充下拉框取消、同名录制及当前文件身份同步的回归测试。
- `版本`：应用、Tauri 与 Rust 包版本统一更新为 `<VERSION>`。

## 下载与校验

- `remember.exe`：Windows x64 优化便携版，<OPTIMIZED_BYTES> bytes（<OPTIMIZED_MIB> MiB）。
- `remember.exe.sha256`：优化版 SHA-256 校验文件。
- `remember-debug.exe`：Windows x64 调试版，仅用于诊断，<DEBUG_BYTES> bytes（<DEBUG_MIB> MiB）。
- `remember-debug.exe.sha256`：调试版 SHA-256 校验文件。

SHA-256：

- `remember.exe`：`<OPTIMIZED_SHA256>`
- `remember-debug.exe`：`<DEBUG_SHA256>`

## 注意事项

- 两个 EXE 均未进行 Authenticode 数字签名，Windows 可能显示“未知发布者”或 SmartScreen 提示；SHA-256 只能校验文件完整性，不能证明发布者身份。发行文件附带 GitHub 构建来源证明，可用于核对其 Actions 构建来源。
- 这是免安装便携版，不是 MSI/NSIS 安装程序。录制保存在 EXE 同级 `recordings` 目录，请把程序放在当前用户可写目录。
- V2 不支持 DPI 或显示缩放变化、任意缩放适配及控件重新布局。V1 和 V2 都会发送真实键盘和鼠标输入，使用前请确认目标窗口和录制来源可信。
- 日常使用 `remember.exe`；`remember-debug.exe` 仅用于问题诊断。
