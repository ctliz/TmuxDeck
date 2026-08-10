# v1.13 TranscriptSource 设计：对话内容从哪来

> 目标：让手机端能看到每个 pane 里 agent 的**分轮次对话内容**，
> 而不是一片滚动的终端屏幕。PRD v1.12 §4 留下的问题在此解决。
>
> 结论先行：**主路径读各 harness 自己的结构化会话记录**（pi / Claude Code 优先），
> `capture-pane` 只做未接入 harness 的兜底。pane → 记录文件的关联
> 不靠猜，靠「文件名里的 intercom 会话 ID」与「cwd 目录 slug」这两条事实。

---

## 0. 与本机事实的对照（2026-08-10 实测）

实现前先把各 harness 的记录格式摸清，设计不基于猜测：

| harness | 记录位置 | 关键事实 |
|---|---|---|
| pi | `~/.pi/agent/sessions/<slug>/<ts>_<uuid>.jsonl` | **文件名里的 `uuid` 就是 intercom 会话 ID**（实测 `019fec77…` 与 `intercom list` 的 session id 一致）。pane 关联因此是精确匹配，不是启发式 |
| Claude Code | `~/.claude/projects/<slug>/<uuid>.jsonl` | 每行 `user`/`assistant` 都带 `cwd` 字段，可用来验证 slug 解析是否正确 |
| Codex | `~/.codex/sessions/YYYY/MM/DD/rollout-<ts>-<uuid>.jsonl` | `session_meta` 里有 `payload.cwd`；桌面版（ChatGPT.app）另在 `~/Library/Application Support/Claude/local-agent-mode-sessions/` |

### 各格式的行样本（实测）

pi（`type: message`，内容在 `message.content[]`，按 `type: text` 过滤）：

```jsonc
{"type": "message", "id": "51dc3b00", "parentId": "…", "timestamp": "2026-08-09T07:40:09.946Z",
 "message": {"role": "user", "content": [{"type": "text", "text": "当前vision客户端…"}], "timestamp": 1786261209943}}
```

Claude Code（`user` 行：用户输入与 `tool_result` 都在 `content[]`；`assistant` 行：
`thinking` / `text` / `tool_use` 混合）：

```jsonc
{"type": "user", "cwd": "/Users/tsiji/Documents/fflink/front", "timestamp": "2026-07-27T13:00:36.416Z",
 "message": {"role": "user", "content": [{"type": "text", "text": "我目前正在做一个机翼…"}]}}
{"type": "assistant", "timestamp": "2026-07-27T13:00:48.569Z",
 "message": {"role": "assistant", "content": [{"type": "text", "text": "我先看一下项目里的文档…"}]}}
```

**要点**：两家的轮次内容都是干净的 `content[].text`，没有转义序列；
工具调用、思考过程是独立 `type`（`tool_use` / `thinking` / `tool_result`），
按需过滤即可。这就是为什么它比 `capture-pane` 和 `pipe-pane` 更合适做「对话」。

---

## 1. pane → 记录文件的关联

`Conversation` 已经带 `cwd` 与 `intercom_session_id`（阶段 1 落地），
关联算法按 harness 分三条路径，全部基于事实：

### 1.1 pi：intercom 会话 ID 精确匹配（首选）

`~/.pi/agent/sessions/<slug>/` 下文件名形如
`2026-08-10T16-18-14-827Z_019fec77-b0ab-….jsonl`，
其中 `uuid` 段以 intercom 会话 ID 开头（实测一致）。

- 路径：`conv.intercom_session_id` → 扫该 cwd 的 slug 目录，
  找 uuid 前缀匹配的文件。**一对一，不会认错。**
- 兜底（无 intercom 时）：同目录下 mtime 最新的 jsonl。

slug 规则（实测）：`"-" + cwd.replace("/", "-") + "-"`。
例：`/Users/tsiji/Documents/TmuxDeck` →
`--Users-tsiji-Documents-TmuxDeck--`。

### 1.2 Claude Code：cwd slug + 内容验证

- 目录：`~/.claude/projects/<slug>/`，slug 规则
  `"-" + cwd.replace("/", "-")`（**无尾缀短横**，与 pi 规则不同，实测）。
- **验证**：目录名本身不可靠（路径里的特殊字符会产生歧义），
  所以扫描候选目录下最新的 jsonl，读前几行，用记录里的 `cwd` 字段
  与 `conv.cwd` 比对确认。目录数量有限（本机 <100），缓存映射。
- 文件选择：同目录下 mtime 最新的 jsonl = 正在进行的会话。
- 多 agent 同 cwd（常见：`vision` 下 3 个 pi 会话）：
  本方案无法在 Claude 侧区分（Claude jsonl 的 uuid 与 pane 无对应关系），
  记入已知边界（见 §5）。

### 1.3 Codex：`session_meta.payload.cwd` 匹配（v1.13 仅关联，不提取）

`~/.codex/sessions/YYYY/MM/DD/` 下按天分目录，滚动文件
`rollout-<ts>-<uuid>.jsonl`。读取 `session_meta` 行的 `payload.cwd`，
与 `conv.cwd` 比对取最新。**v1.13 只实现关联探测，轮次提取未实现**
（Codex 记录的 `developer` role 含大量系统提示，提取规则需要单独设计），
先走 capture-pane 兜底。桌面版 Codex 记录在应用沙箱目录，同样不做。

### 1.4 未覆盖 harness（Aider / Gemini CLI / 纯 shell）

无结构化记录 → 直接走 `CapturePaneSource` 兜底（已在 `bridge.rs`）。

---

## 2. 轮次提取规则

| 记录行 | 角色 | 提取 |
|---|---|---|
| pi `message.role == user`，`content[].type == text` | `Human` | `text` |
| pi `message.role == assistant`，`content[].type == text` | `Agent` | `text` |
| Claude `type == user`，`content[].type == text`（`isMeta`/`isCompactSummary` 排除） | `Human` | `text` |
| Claude `type == assistant`，`content[].type == text` | `Agent` | `text` |
| `thinking` / `tool_use` / `tool_result` / `attachment` | — | **丢弃**（v1.13 不展示，避免手机端被文件内容刷屏；后续版本可加开关） |
| `custom_message` / `system` / `compaction` | `System` | 仅 `compaction` 汇总文本可选用 |

时间戳：两家都是 RFC3339（`2026-08-09T07:40:09.946Z`），
`Turn.timestamp` 统一转毫秒。不引 chrono——写一个只认
`YYYY-MM-DDTHH:MM:SS[.mmm]Z` 的极小解析器（本机文件全部由
`new Date().toISOString()` 产生，格式固定）。

---

## 3. 增量拉取：字节游标

`TranscriptSource::poll(conv, since)` 的语义按「文件追加日志」实现：

- 每个关联文件维护 `(path, byte_offset)` 游标（`HashMap`）。
- 每次 poll 从游标处续读，**不重扫整个文件**；解析新行，过滤
  `timestamp > since`，产出 `Turn`，推进游标。
- 文件被轮转/压缩（Claude compaction、pi 归档）导致
  `file_len < cursor` 时重置游标从头读，靠 `since` 去重。
- 文件消失（agent 重启换新文件）：重新走 §1 关联，游标归零，
  但 `since` 保证不重复推送旧轮次。

---

## 4. 优先级链（Composite）

```
CompositeTranscriptSource::poll(conv, since)
 ├─ kind=Pi          → PiTranscriptSource（intercom 会话 ID 精确匹配）
 ├─ kind=ClaudeCode  → ClaudeTranscriptSource（cwd 扫描验证）
 └─ 其余（Codex/Shell/Unknown/匹配失败）→ CapturePaneSource（整屏兜底）
```

分层有日志：`transcript[%3] pi → file.jsonl (cursor 1234)`，
`transcript[%3] no structured record, fallback capture-pane`。
手机端不知道也不关心走的是哪条路。

---

## 5. 边界与已知坑

1. **Claude 桌面版（Claude.app）**：记录在 `~/Library/Application Support/Claude/
   local-agent-mode-sessions/`，uuid 与 pane 无对应关系。
   v1.13 只覆盖 CLI 的 `~/.claude/projects/`；桌面版按「最新 mtime + cwd」尽力而为，
   认不准就兜底。
2. **多 agent 同 cwd**：pi 靠 intercom ID 精确区分；Claude/Codex 无法区分，
   只能取最新文件。若同一 cwd 开多个 Claude 会话，内容会串——验收时明确此限制。
3. **pi 的 `compaction`**：会把历史压缩进一条记录，游标重置后靠 `since` 去重，
   不会被重复推送。
4. **时间戳相同**：Claude 有时多行同一毫秒；游标按行推进保证不漏，
   排序以文件顺序为准，不重排。
5. **文件权限**：`~/.claude/projects/**/*.jsonl` 是 `0600`（实测），
   但 TmuxDeck 与 Claude 同用户运行，无碍。

---

## 6. 验收清单

单元测试（合成 jsonl 夹具，写临时目录）——全部通过（`cargo test`，27 项）：

- [x] pi：文件名 uuid 前缀匹配 intercom 会话 ID → 唯一文件
- [x] pi：`user`/`assistant` text 提取为 Human/Agent，`tool_call` 忽略
- [x] Claude：slug 目录定位 + `cwd` 字段验证；`thinking`/`tool_use`/`tool_result` 忽略
- [x] 增量：第二次 poll 只返回新行（游标推进正确）
- [x] 文件轮转：游标重置 + `since` 去重，不重复推送
- [x] RFC3339 → 毫秒解析器（含无毫秒、有毫秒）
- [x] 同毫秒多行不丢不漏（last_seen 去重）
- [x] Composite：无记录时 pi 分支不 panic，走兜底
- [x] 真机：当前 pi 会话 resolve 到正确 jsonl，提取出本对话轮次（`cargo test -- --ignored`）

真机验证（需在跑 pi + Claude Code 的机器上）：

- [ ] 对当前 pi 会话 poll 出本对话的轮次（与 `intercom list` 的会话对应）✅ 已验证
- [ ] 对正在跑 Claude Code 的 pane poll 出该项目的对话（本机暂无 CLI 模式 Claude 在跑）
- [ ] agent 在说话时（thinking）能拿到最新文本；tool 执行时不产生噪音轮次
