# Yuumi

Tauri v2 + Vue 3 + TypeScript 桌面应用。
原版 Python (Seraphine) 项目的 Rust 重构。

## 编码指导原则 (Coding Guidelines)

**权衡：** 这些原则倾向于谨慎而非速度。对于微不足道的任务，请自行判断。

### 1. 编码前思考 (Think Before Coding)

**不要假设。不要隐瞒困惑。展现权衡。**

- **明确陈述你的假设**：如果不确定，请进行询问。
- **展现多种解释**：如果存在多种解释，请展示它们 —— 不要默默选择其中一种。
- **提倡更简单的方法**：如果有更简单的方法，请指出。在必要时进行反驳/建议。
- **停止并提问**：如果有不清楚的地方，请停下来，指出令人困惑的点，并进行询问。

### 2. 简洁第一 (Simplicity First)

**用最少的代码解决问题。不做任何投机性的编写。**

- **不要添加超出需求的功能**。
- **不要为单次使用的代码进行抽象**。
- **不要添加未要求的“灵活性”或“可配置性”**。
- **不要为不可能发生的情景编写错误处理**。
- **如果写了 200 行代码，而 50 行就能搞定，请重写**。
- **思考**：问问自己：“资深工程师会觉得这太复杂了吗？”如果是，请简化。

### 3. 外科手术式修改 (Surgical Changes)

**只改必须改动的。只清理你自己的烂摊子。**

- **不要“改进”相邻的代码、注释或格式**。
- **不要重构没有损坏/正常工作的代码**。
- **匹配现有的代码风格**，即使你有不同的习惯。
- **对于不相关的死代码，请提及 —— 不要直接删除它**。
- **清理引入的无用代码**：移除因**你的**修改而不再使用的 imports、变量或函数。
- **不要移除预先存在的死代码**，除非被明确要求。
- **检测标准**：修改的每一行都应该能直接追溯到用户的需求。

### 4. 目标驱动执行 (Goal-Driven Execution)

**定义成功标准。循环直到验证通过。**

将任务转化为可验证的目标：

- “添加校验” → “为无效输入编写测试，然后使其通过”
- “修复 Bug” → “编写复现该 Bug 的测试，然后使其通过”
- “重构 X” → “确保重构前后测试均能通过”
- **多步骤任务需陈述简短计划**：
  ```
  1. [步骤] → 验证: [检查项]
  2. [步骤] → 验证: [检查项]
  3. [步骤] → 验证: [检查项]
  ```
- **成功标准**：强大的成功标准能让你独立循环。弱标准（“使其工作”）需要不断澄清。

## 完成定义 (Definition of Done)

**改完代码必须跑校验，禁止只改不验。**

| 变更范围 | 最低验证 |
| :--- | :--- |
| 任意 Rust / TS / Vue | `pnpm check-all` |
| 仅前端逻辑（映射/统计/缓存等） | `pnpm type-check` + `pnpm test:unit` |
| 新增 / 重命名 Tauri 命令 | `pnpm check:commands`（定义 / 注册 / 前端调用三方一致） |
| 格式 / Clippy 疑问 | `pnpm format` → `pnpm clippy` |

`check-all` 当前覆盖：`check:commands` → `format` → `type-check` → `lint` → `test:unit` → `fmt-check` → `clippy`。

## Tauri v2 & Vue 3 编码规范 (Tauri & Vue Guidelines)

### Rust 后端 (Tauri v2 / Rust)

- **类型安全边界**：所有与前端交互的 Struct/Enum 必须实现 `serde::Serialize` 和 `serde::Deserialize`。
- **错误传播与序列化**：
  - `#[tauri::command]` 如果可能失败，必须返回 `Result<T, String>`。
  - 严禁随意使用 `unwrap()` 或 `panic!`，应使用 `map_err(|e| e.to_string())` 或 `thiserror` 将 Error 转化为前端友好的 String，并使用系统 `logging.rs` 的 logger 记录完整堆栈。
- **LCU 静态资源与自定义协议规范**：
  - LCU 静态资源（英雄、技能、物品、符文、战利品等图标）由 Rust 端的 `yuumi-asset://` 自定义协议流式返回原始字节，彻底绕开 IPC 与 Base64 传输。
  - WebView2 仅拦截 http/https 协议，前端图片 URL 使用 wry workaround 前缀 `http://yuumi-asset.localhost/`（host 含协议名），由 wry 自动还原为 `yuumi-asset://localhost/` 后再交给 Rust handler。
  - `get_lcu_asset` / `get_lcu_assets` 仅作兼容层保留，新功能**严禁**通过 IPC 命令读取图片 Base64。
- **共享状态管理与死锁防护**：
  - 只能通过 `tauri::State<'_, AppState>` 访问全局状态，不得使用不安全的全局静态变量。
  - 在异步 Command 或后台 Task 中获取状态锁时，**严禁跨 `await` 点持有同步锁 (`std::sync::MutexGuard`)**，必须先释放锁或在作用域块分离后再 `await`，防止 Tokio 线程池死锁。
  - `AppState` 按域聚合在 `state.rs`；LCU 连接与预加载数据在 `lcu` 运行时字段中，不要另开全局静态。
- **异步与非阻塞**：
  - 严禁在 Command 的主线程中执行耗时的 CPU 计算或 I/O 操作。
  - 使用 `tokio::spawn` 投递后台任务，并在执行完毕后通过 `tauri::Emitter::emit`（Tauri v2 API，禁止使用 v1 的 `emit_all`）异步通知前端。
- **Tauri 命令规则**：
  1. 新增 `#[tauri::command]` 必须在 `lib.rs` 的 `invoke_handler!` 中注册，否则前端调用会提示 command not found。
  2. **以 `lib.rs` 的 `generate_handler!` 列表为唯一真源**；不要在本文档维护命令大表。
  3. 注册后必须跑 `pnpm check:commands`（校验定义 / 注册 / 前端 `invoke` 三方一致）。
- **便携版兼容**：数据目录由 `runtime.rs` 决定——安装版 `%APPDATA%/Yuumi`，便携版（exe 旁有 `portable.flag`）为 exe 同级 `data/`。路径相关逻辑必须走 `runtime::app_data_dir()`，禁止硬编码 `%APPDATA%`。

### Vue 3 前端 (Vue 3 / TypeScript)

- **类型约束与强类型收敛**：
  - 必须对所有 `invoke` 的入参及返回结果定义明确的 TypeScript Interface，绝对禁止使用 `any`。
  - 组件 `defineProps`、Composable 函数参数与返回值、Pinia Store 状态严禁随意使用 `any`；对于具备多形态或阶段性差异的数据结构（如选人与游戏内玩家对象、组队结构），必须在 `src/types/` 下统一定义结构明确的 Interface 或联合类型。
  - 严格处理可选属性（`undefined` / `null`）的空值守卫：在进行函数参数传递、数学运算、数组查找或对象动态 key 索引（如 `playerData[key]`）前，必须通过可选链、空值合并操作符（`??`）或类型守卫完成安全收敛，防止类型逃逸与运行时错误。
  - Rust 返回的 Result 应该在前端有合理的错误捕获（`try-catch` 或 `.catch()`），并通过 `useToast` 或 `message` 呈现给用户。
- **事件监听生命周期管理**：
  - 使用 `@tauri-apps/api/event` 的 `listen` 订阅 Rust 事件时，必须在组件销毁时（`onUnmounted`）调用返回的 `unlisten()` 函数，以防闭包内存泄漏。
- **LCU API 隔离原则**：
  - 前端绝不应直接建立与 LCU 端口的 HTTP/WebSocket 连接。
  - 所有 LCU 接口的调用，必须经由 Rust 端的 `call_lcu_api` 转发，以规避 Token 泄漏并统一错误捕获。
- **静态图片资源渲染规范**：
  - 所有 LCU 静态资源统一使用 `<LcuImage :src="path" />` 或 `useLcuAsset`（输出 `http://yuumi-asset.localhost/...` 协议 URL，**不是** data URL / Base64）。
  - 资源加载由 Chromium 原生网络层发起，自带 HTTP 强缓存与原生解码，**严禁使用 IPC `invoke` 批量传递图片 Base64 字符串**。
- **页面状态保留**：
  - 路由基于手动 `currentPage` ref 切换，不是 vue-router。
  - **所有业务页**（Search / GameInfo / Career / TFT / Settings / Tools / SavedPlayers）使用 `v-show` 保持挂载，并配合 `visitedPages` 延迟首次挂载，避免启动即拉大数据。
  - 新增页面时在 `src/constants/appPages.ts` 注册元数据，不要改回 `v-if` 整页销毁。
- **多窗口安全**：
  - 存在主窗口与 OP.GG 独立窗口（入口 `main.ts` / `opgg.ts`）。窗口相关 API（toast、全局 message、主窗口专用事件）必须能降级，参考 `useToast.ts` 的多窗口处理。
- **i18n**：
  - 用户可见文案走 `vue-i18n`（`src/i18n.ts`），不要散落硬编码中文字符串；日志、开发者调试信息除外。
- **主题与样式**：
  - 遵循 "纯白水晶极光" 风格，背景使用毛玻璃模糊（`backdrop-filter: blur`），配色统一采用动态 CSS 变量（`styles/theme-tokens.css`），不可随意硬编码色值。
- **单元测试**：
  - 纯逻辑（映射、统计、缓存、队伍计算等）优先放在 composables / utils 并补 vitest 用例（`src/composables/__tests__/`）。
  - 运行：`pnpm test:unit`。

## 技术栈

- **前端**: Vue 3 + TypeScript + Vite + Pinia + Naive UI + vue-i18n + vitest
- **后端**: Tauri v2 (Rust)
- **包管理**: pnpm

## 常用命令

```bash
pnpm tauri dev      # 开发（Vite + Tauri 窗口）
pnpm tauri build    # 构建生产包
pnpm dev            # 仅前端开发
pnpm build          # 仅前端构建

# 代码质量校验（修改代码后必须执行，见「完成定义」）
pnpm check:commands  # Tauri 命令三方一致性
pnpm type-check      # Vue / TS 类型检查
pnpm lint            # ESLint
pnpm test:unit       # 前端 vitest
pnpm format          # cargo fmt
pnpm fmt-check       # cargo fmt --check
pnpm clippy          # cargo clippy -D warnings
pnpm test:rust       # cargo test
pnpm check-all       # 一键全量检查
```

## 项目结构（导航图，非完整清单）

```
Yuumi/
├── src/                          # Vue 前端
│   ├── App.vue                   # 根组件：标题栏 + 侧栏 + store.currentPage 路由（v-show + appPages 元数据）
│   ├── main.ts / opgg.ts         # 主窗口 / OP.GG 独立窗口入口
│   ├── i18n.ts                   # vue-i18n 配置
│   ├── api/                      # Rust 命令封装（lcu/ 目录按域 + loot.ts）
│   ├── store/                    # Pinia：lcuStore（LCU 事件）+ gameInfoStore（对局玩家运行时）
│   ├── types/                    # 跨组件共享 Interface（lcu/gameInfo/search/opgg）
│   ├── styles/                   # 主题 token、基础样式、Naive UI 覆盖
│   ├── composables/              # 业务 Hook + __tests__（vitest）
│   ├── utils/                    # 纯工具（theme/缓存/并发/queueMeta/gameDetailBuilder 等）
│   ├── constants/appPages.ts     # 保活页面元数据（v-show + 延迟挂载）
│   ├── views/                    # 页面（Home/Career/Search/GameInfo/SavedPlayers/TFT/Settings/Tools/BenchOverlay）
│   └── components/               # 按域分子目录：tft/ gameinfo/ career/ tools/ settings/ search/ opgg/ layout/
│                                 # 共享：LcuImage、ChampionPicker、SpellPicker、NaiveApiCapture 等
├── src-tauri/
│   ├── src/
│   │   ├── main.rs / lib.rs      # 入口；AppState 装配、Agent 启动、托盘、invoke_handler
│   │   ├── runtime.rs            # 便携版识别与 app_data_dir
│   │   ├── state.rs              # AppState 按域聚合（含 SignalrRuntime）
│   │   ├── config.rs             # 配置读写 + Schema 迁移
│   │   ├── logging.rs            # 自研日志（按天轮转 + 单文件 2MB 分片）
│   │   ├── saved_players/        # 路人集（db/types/encounters/commands/import_export）
│   │   ├── auto_tag.rs           # 对局结束自动打标（纯逻辑 + 单测）
│   │   ├── portable_updater.rs   # 便携版 zip 更新
│   │   ├── updater.rs            # 安装版更新
│   │   ├── signalr.rs            # SignalR Hub 远程反代（状态在 AppState.signalr）
│   │   ├── lcu_ops.rs            # LCU 业务工具（房间/摇号/符文/皮肤/观战/设置读写）
│   │   ├── commands/             # config / lcu / os_shell（系统/启动/GitHub）
│   │   ├── lcu/                  # monitor / client / ws / opgg / sgp / game_data
│   │   ├── parsers/              # summoner / match_parser/ / game_info / tft/
│   │   │   └── match_parser/     # types+queue_time + display + history + teammates
│   │   ├── agents/               # auto_bp / auto_match/ / auto_screenshot
│   │   │   └── auto_match/       # mod(事件循环) + helpers + flows + honor + post_game
│   │   ├── loot/                 # open / inventory / actions
│   │   └── upload/               # queue / trigger / store / payload / service / url / tests
│   ├── tauri.conf.json
│   └── capabilities/
├── scripts/check-commands.mjs    # 命令三方一致性校验
├── package.json
└── AGENTS.md
```

> 结构树会腐烂。新增顶层模块时补一行即可；不要展开每个 `.vue` / `.rs`。

## 架构数据流

```
LeagueClientUx.exe
  ↓ (sysinfo 轮询 port/token；lockfile / 命令行 / WMIC 兜底)
monitor.rs → AppState.lcu + game_data（英雄/物品/技能/符文预加载）
  ↓
ws.rs → LCU WebSocket（新连接自动取消旧循环）
  ├─→ 前端: Tauri emit → lcuStore.ts → Vue 组件
  ├─→ BP Agent: mpsc → auto_bp.rs（自动选人/禁人/技能/板凳席）
  ├─→ Match Agent: mpsc → auto_match.rs（自动接受/邀请/点赞/再来一局/重连）
  │     └─→ 游戏内状态 → auto_screenshot.rs（多杀截图）
  │     └─→ 对局结束 → auto_tag.rs 打标 + UploadTrigger
  └─→ UploadQueue → 外部 API（失败落盘 pending_uploads 重试）
```

### GameInfo 对局信息页数据源

```
lcuStore（phase / champSelectSession / gameflowSession）
        ↓
useGamePlayerData（编排）
  ├─ champSelectSnapshot     选人签名 / bot / 自定义 cell
  ├─ gameflowTeamPipeline    session → 我/敌 → 合并 → 拉取 → live 兜底
  ├─ playerDetailLoader      单人：身份/战绩/段位/熟练度/宿命
  └─ gameInfoStore           主键 puuid（pending:cell）+ cell/sid 别名
        ↓
GameInfo.vue → findPlayerData → PlayerInfoCard
旁路：querySavedPlayersMap（路人标记）、computePremadeColors（预组队色）
```

| 阶段 | 队伍列表 | 玩家详情 |
|------|----------|----------|
| ChampSelect | gameflow* → session.my/theirTeam → champSelect*Snapshot | 逐人 loadPlayerData |
| GameStart / InProgress | gameflow*（身份合并后） | 同上 + live teams 补敌方 |
| 非对局（保留盘开） | localStorage 恢复 | restorePlayers |

视图读取统一走 `findPlayerData` / `store.getPlayer`，不要在组件里再扫多键。

## 窗口与 UI

- **自定义标题栏**: `decorations: false`；最小化/最大化/关闭 + 返回导航 + 游戏阶段显示
- **系统托盘**: 主页/生涯/战绩查询/对局信息/TFT/其他功能/设置/退出；支持关闭到托盘
- **主题**: 纯白水晶极光，CSS 变量 + 毛玻璃 + 自定义滚动条
- **页面路由**: `store.currentPage` 为唯一真源；业务页用 `v-show` + `visitedPages` 延迟挂载（`constants/appPages.ts`）
- **多窗口**: 主窗口 + OP.GG 独立窗口；公共逻辑避免假设仅主窗口

## 后台 Agents

| Agent | 触发 | 职责 |
| :--- | :--- | :--- |
| `auto_bp` | `/lol-champ-select/v1/session` | 自动选人/禁人/技能/板凳席推送 |
| `auto_match` | gameflow-phase + ready-check | 模块目录：事件循环 + 自动接受/邀请/再来一局/报边 + 对局后缓存/打标/雷达/提醒 |
| `auto_screenshot` | 游戏内多杀事件（由 auto_match 切换 in-game） | 按档位自动截图到用户目录 |

## 关键子系统（读代码入口）

| 子系统 | 入口 | 要点 |
| :--- | :--- | :--- |
| 配置 | `config.rs` + `runtime.rs` | PascalCase JSON；`Version` + `migrate`；便携/安装数据目录 |
| LCU 客户端 | `lcu/client.rs` | 忽略 SSL、代理、Basic Auth、超时；CDragon 代理 |
| 对局数据 | `parsers/match_parser.rs` `game_info.rs` | 战绩清洗、10 人段位/KDA、宿命分析 |
| 云顶 | `parsers/tft/*` + `components/tft/` | LCU 优先、CDragon 兜底；OP.GG 热门阵容 |
| 战利品 | `loot/*` | 批量开箱、分解、重铸、精粹 |
| 路人集 | `saved_players/` | SQLite、标签、相遇历史、导入导出、身份回填 |
| 自动打标 | `auto_tag.rs` | 表现评分 → 标签；有单元测试 |
| 上传 | `upload/*` | 去重队列、阶段触发、Smart Split、失败重试 |
| 更新 | `updater.rs` / `portable_updater.rs` | 安装版 vs 便携 zip 覆盖 |

## 配置文件

- 安装版：`%APPDATA%/Yuumi/config.json`
- 便携版：exe 同级 `data/config.json`
- 访问必须通过 `runtime::app_data_dir()` / 配置 API

顶层字段（JSON 为 PascalCase）：

- `Version` — Schema 版本，破坏性变更时递增并迁移
- `General` — 客户端路径（含 WeGame）、启动、HTTP 代理、日志级别、上传 API、SignalR
- `Personalization` — Mica、DPI、语言、主题色、胜负/Remake 卡片色
- `Functions` — 自动化开关与候选列表（BP/技能/流程/截图/上传/侧栏显隐/自动打标敏感度等）
- `Other` — 杂项（公告 SHA、搜索历史等）

## 开发与调试

- **前端调试**: 开发模式自动打开 Chromium DevTools，`F12` 可审组件与网络。
- **日志**: 自研 `logging.rs`（`log` crate），按天轮转 + 单文件 2MB 分片，保留 30 天；路径为 **exe 同级** `log/`（非 `%APPDATA%`）。
- **排查 LCU / 后台 Task**: 在最新 `.log` 中检索 `[LCU]`、`[WS]`、`[Error]`。
- **命令对不上**: 先跑 `pnpm check:commands`，不要靠肉眼对表。
