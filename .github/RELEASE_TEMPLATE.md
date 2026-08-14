## 本次改动

- `新增`：列出新增的用户功能、界面或工具。
- `重构`：列出结构、流程或交互调整。
- `优化`：列出性能、稳定性或资源占用改进，并给出必要的量化结果。
- `修复`：列出已解决的用户可见问题。
- `移除`：列出删除的功能、入口或旧行为。

## 下载与校验

- `remember.exe`：Windows x64 优化便携版。
- `remember.exe.sha256`：优化版 SHA-256 校验文件。
- `remember-debug.exe`：Windows x64 调试版，仅用于诊断。
- `remember-debug.exe.sha256`：调试版 SHA-256 校验文件。

SHA-256：

- `remember.exe`：`<SHA-256>`
- `remember-debug.exe`：`<SHA-256>`

## 注意事项

- 说明签名状态、系统兼容性和必要的使用限制。
- 日常使用优化版；Debug 版仅用于问题诊断。

## 发布流程

1. 同步更新 `package.json`、`src-tauri/Cargo.toml` 与 `src-tauri/tauri.conf.json` 的版本，并更新 `.github/RELEASE_NOTES.md`。
2. 通过 Pull Request 合并到 `main`。Windows CI 必须完整通过测试、覆盖率、RustSec、真实输入往返、构建与来源证明步骤。
3. CI 从同一次运行生成优化版、Debug 版及两个 SHA-256 文件，并自动创建对应 `v<版本>` GitHub Release。
4. 不得把本地构建的 EXE、校验文件或修改后的 CI 下载文件上传到正式 Release；需要重试时重跑同一提交的 Actions，或提交新的版本修正。
