# Yuumi 全项目 Superpowers Code Review

日期：2026-03-19  
范围：Rust 后端（src-tauri/）、Vue 前端（src/）、架构与安全、自动化校验

## 0. 自动化校验结果

| 检查 | 结果 |
|------|------|
| `pnpm check:commands` | 通过（72 定义 / 72 注册 / 69 invoke；6 条未使用警告） |
| `pnpm type-check` | 通过 |
| `pnpm lint` | 0 error / 18 warning（`AutoHoverCard.vue` 事件连字符） |
| `pnpm test:unit` | 12 files / 65 tests 全过 |
| `pnpm clippy -D warnings` | 通过 |
| `pnpm fmt-check` | 通过 |
| `cargo test` | 48 tests 全过 |

结论：**工程基线健康**，无阻断构建/测试失败。问题集中在安全边界、信息泄漏与规范偏离。

---

## Critical（建议立即修复）

### C1. 诊断文件明文写入 LCU Token — **已修复**
- **位置**：`src-tauri/src/lcu/monitor.rs:97-116`
- **证据**：未找到 LCU 时将 `p.cmd()` 原文写入 `%TEMP%/yuumi_lcu_debug.txt`；同文件日志路径 `:338` 已走 `sanitize_cmdline`，文件路径没有。`--remoting-auth-token=` 会原样落盘。
- **影响**：本机其他进程/用户可读临时文件拿到 LCU 凭据。
- **修复**：写诊断文件前同样 `sanitize_cmdline`。新增单测 `sanitize_cmdline_masks_token` / `sanitize_cmdline_keeps_plain_cmd`。

### C2. SignalR 日志泄漏 connectionToken — **已修复**
- **位置**：`src-tauri/src/signalr.rs:227-236`
- **证据**：`log::info!("正在建立 WebSocket 连接到: {}", ws_url)`，URL 含 `id={connectionToken}&userId={user_id}`。
- **影响**：日志文件（exe 同级 `log/`，保留 30 天）可被用于重连 Hub。
- **修复**：日志只打 `{ws_base}/lcuHub`，不再带 query。

### C3. `is_local` 用 `contains` 导致远程 Hub 跳过证书校验 — **已修复**
- **位置**：`src-tauri/src/signalr.rs:238, 300` + TLS 配置 `:240-245, 285-297`
- **证据**：`server_url.contains("127.0.0.1") || contains("localhost")`；配置形如 `https://evil.example/?x=127.0.0.1` 会被判为 local，从而对**远程** Hub 使用 `NoVerifier` / `danger_accept_invalid_certs`。
- **影响**：MITM 风险。
- **修复**：新增 `is_local_server_url`，用 `url::Url` 解析 host，IPv4/IPv6 走 `is_loopback()`，域名只认 `localhost`（忽略大小写）。新增 3 个单测覆盖本地/远程/非法 URL。

---

## Major（应尽快处理）

### M1. `lcu_request` 白名单可被 `..` 绕过 — **已修复**
- **位置**：`src-tauri/src/lcu/client.rs`
- **修复**：新增 `is_api_path_allowed`，拒绝 `..`、`//`、空路径；新增单测。

### M2. `resolve_asset` 开放任意 `http(s)://` → SSRF 面 — **已修复**
- **位置**：`src-tauri/src/lcu/client.rs`
- **修复**：`is_allowed_cdn_url` 仅允许 `raw.communitydragon.org`；新增单测。

### M3. TFT 缓存文件名未防路径逃逸 — **已修复**
- **位置**：`src-tauri/src/lcu/client.rs`
- **修复**：`sanitize_cache_file_name` 白名单 `[a-zA-Z0-9_.-]`，拒绝 `..`/路径分隔符；新增单测。

### M4. CSP 允许 `script-src 'unsafe-inline'` + 多处 `v-html` — **已修复（需本机冒烟）**
- **位置**：`src-tauri/tauri.conf.json`
- **修复**：去掉 `script-src 'unsafe-inline'`，保留 `style-src 'unsafe-inline'`（Naive UI / 组件样式仍需要）。`v-html` 继续经 DOMPurify。
- **验证**：`check-all` 通过；**请本机 `pnpm tauri dev` 确认页面脚本正常**（Tauri 注入与 Vite 产物若异常需回退本条）。

### M5. 生产窗口始终开启 DevTools — **已修复**
- **位置**：`src-tauri/tauri.conf.json`
- **修复**：`"devtools": false`（debug 仍由 `lib.rs` 自动打开）。

### M6. 建厅标志先置 true，失败后永不自动重试 — **已修复**
- **位置**：`src-tauri/src/agents/auto_match/flows.rs`
- **修复**：30 次失败、LCU 断开时 `created = false`。

### M7. 大量模块绕过 `lcu_request` 统一入口 — **暂缓（架构债）**
- **涉及**：`loot/*`、`lcu_ops.rs`、`upload/*`、`auto_screenshot.rs`、`match_detail` 等
- **状态**：涉及面广、需回归自动化行为；建议独立迭代逐步收敛，不在本轮强行改。

### M8. 前端用户可见文案大面积硬编码中文 — **大部分修复**
- **已完成**：队列映射收敛；雷达 toast；对局详情胜/败方与目标 title；OP.GG 符文 toast；拉黑 dialog；设置页路径/WeGame/截图 toast；App 配置错误 dialog。
- **刻意保留中文**：`电脑` / `玩家{n}` 等 LCU 人机命名——`gamePlayerStats` 依赖其做识人，翻译会破坏检测。
- **未完成**：零星组件内注释级/次要 tooltip，可后续扫尾。

### M9. `window.prompt` 原生弹窗 — **已修复**
- **位置**：`src/components/gameinfo/PlayerInfoCard.vue`
- **修复**：改 Naive `dialog.create` + `NInput`，取消与空理由语义正确。

### M10. 前端类型/资源规范零星偏离 — **已修复**
- `OpggModal.vue`：`invoke<GameDataAssets>`。
- `TftMetaCompsTab.vue`：棋盘/装备改 `LcuImage`。

### M11. 更新源含第三方代理 `ghp.ci` — **已修复**
- **位置**：`src-tauri/tauri.conf.json`
- **修复**：仅保留 GitHub 官方 endpoint。

---

## Minor（可择机处理）

| ID | 问题 | 状态 |
|----|------|------|
| m1 | `opener:default` 下放到全部窗口 | **已修复**：仅 `main.json` 保留 opener |
| m2 | 配置加载不跑 `validate()` | **已修复**：load 后 soft-validate 并 warn |
| m3 | 自定义协议可返回 `image/svg+xml` | 暂缓，需运行时确认导航面 |
| m4 | 已注册未使用命令 3 个 | 保留（兼容/预留），不删除 |
| m5 | 已 emit 未监听事件 3 个 | 保留（调试/预留），不删除 |
| m6 | 应用级 `listen` 不保存 unlisten | 风险低，暂缓 |
| m7 | `lcuStore` 事件 payload 仅 `as` 断言 | 暂缓（边界校验可后续加） |
| m8 | `api/lcu/assets.ts` 兼容层死代码 | **已标记 `@deprecated`** |
| m9 | 超大 Vue 文件 | 暂缓，独立重构迭代 |
| m10 | `AppState::lcu()` footgun | 暂缓，多数调用方已正确 |
| m11 | ESLint 连字符 warning | **已修复**（eslint --fix） |
| m12 | AGENTS.md 轻微腐烂 | **已修复**（match_parser 路径 + pipeline_stats） |
| m13 | 测试缺口 | **部分补齐**：路径穿越 / CDN host / TFT 文件名 / sanitize / is_local |

---

## 做得好的点

- **锁跨 await 防护意识强**：`lcu_params`、signalr/ws/screenshot 多处实现与注释一致。
- **Token 日志脱敏主路径扎实**：`sanitize_cmdline`、lockfile/WMIC debug（诊断文件除外，见 C1）。
- **便携 zip 解压安全**：`enclosed_name` + 根目录校验 + 符号链接拒绝（`portable_updater.rs`）。
- **配置原子写 + 损坏备份**（`config.rs`）。
- **前端无 `any` 滥用**；api 层普遍 `invoke<T>` + try/catch + toast。
- **资源主路径正确**：`LcuImage` / `yuumi-asset` 协议，未发现主动 Base64 传图。
- **页面保活**：`v-show` + `visitedPages` 符合规范。
- **测试**：前端 66 + Rust 56，覆盖 auto_tag、upload、display/history、portable_updater、saved import_export 等核心纯逻辑。
- **SQLite**：参数化 SQL + `spawn_blocking`。
- **`spawn_log_panic`** 捕获后台任务 panic。

---

## 本轮修复后仍建议跟进

1. **M4 CSP 冒烟**：本机启动确认无脚本被 CSP 拦截。
2. **M7 架构收敛**：高频 LCU 调用逐步并入 `lcu_request`（涉及 loot/upload/lcu_ops 等，独立迭代）。
3. **m9**：拆 MatchHistoryTab / LootManagerTab / PlayerInfoCard 超大组件。
4. **m6/m7**：应用级 listen unlisten、lcuStore payload 字段校验。

---

## 未发现问题的领域（抽查通过）

- 前端无 `any` / `as any`
- 未发现 Base64 IPC 传图
- 业务页未误用 `v-if` 整页销毁
- 上传 payload 无密码/token（隐私数据符合功能预期）
- Capabilities 主体：仅 `main` 可 `create-webview-window`
- 依赖 lockfile 完整，无明显危险废弃 crate
