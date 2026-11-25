# AGENTS.md

## 工作流

- 使用 Makefile 定义的指令，完成构建、测试、运行、清理构建缓存等工作
- 如果默认端口被占用，则自行换一个可用端口再启动，同时保持前后端端口配置一致
- 注意查看tron-frontend和tron-backend里的嵌套的AGENTS.md

## 开发共识

- 后端 JSON 响应统一使用 `ApiResponse<T>` [ `tron-backend/src/response.rs` (`code`, `message`, `data`)]。每个http接口使用独立的DTO定义，不要混用
- 不要在项目内新增技术设计文档、脑爆文档、产品设计等过程文档
- 发布新的正式版都要修改CHANGELOG和推送tag，tag不要带"v"前缀

## 国际化（i18n）

- 后端用户可见文案统一存放在 `tron-backend/locales/*.yml`（rust-i18n `_version: 2`），业务代码只通过 `tron_i18n::t!("语义化 Key", locale = locale)` 引用，禁止硬编码中英文文案对（包括 `locale.text("中文", "English")` 和按语言分支写死的字符串）。
- 新增或修改文案必须同时补齐 `zh-CN` 与 `en`，提交前运行 `make be-i18n-check`；CI 会拒绝缺失翻译或未定义的 Key。
- 请求、会话、Agent 运行的语言必须显式传入（`Accept-Language`、会话 locale、`AgentRunContext.locale`），不要依赖进程级全局语言设置，避免并发会话互相影响。
- 业务层返回结构化类型（如风险原因、错误码），由展示层负责翻译；不要在领域逻辑里拼接本地化文本。


## SQLite 空间回收（改动持久化前必读）

- 业务库统一经 `framework_storage::SharedDatabase::open` 打开，新库在建表前启用 `auto_vacuum=INCREMENTAL`。旧库在后台巡检中使用同一个写连接设置模式并执行一次性 `VACUUM`，磁盘余量不足或转换失败时留待下轮。
- HTTP 服务启动后延迟 10 分钟开始巡检，每轮结束后等待 10 分钟。已启用增量回收的业务库每轮最多回收 1024 页。
- 若新增 SQLite 文件绕过 `SharedDatabase`（直接 `rusqlite::Connection::open`），需自行补同样的 PRAGMA 与回收，否则文件只增不减。
