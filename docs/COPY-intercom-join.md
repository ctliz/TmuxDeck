# Intercom Join & Workspace Concepts Copy (Final)

## 1. 短提示 (Short Hints)

### 中文 (Chinese)
• 工作区名就是卡片标题，也是 tmux 会话名。
• /name 只改圈内昵称，不改工作区，也不改会话 ID。
• join 只加入该工作区的通话圈，不会成为 Team Worker。

### English
• Workspace name is the card title and the tmux session name.
• /name only sets your in-circle nickname; it does not rename the workspace or session ID.
• join adds you to that workspace’s intercom circle; it does not make you a Team Worker.

---

## 2. CLI 成功输出 (CLI Success Output)

### 中文 (Chinese)
已加入工作区 {workspace} 的通话圈。
身份：外部协作者（不是 Team Worker）
你的显示名：{name}

### English
Joined the intercom circle for workspace {workspace}.
Role: external collaborator (not a Team Worker)
Display name: {name}

---

## 3. 帮助弹窗 (Help Modal / FAQ)

### 中文 (Chinese)
**工作区与通话圈说明**
1. **工作区名**：就是卡片标题，也是底层的 tmux 会话名。用于指定你要加入哪一个通话圈。
2. **圈内昵称 (/name)**：使用 `/name` 只修改你在该通话圈内的显示名，不会改动工作区名，也不会修改会话 ID。
3. **加入通话圈 (join)**：通过 `join` 加入对应工作区的通话圈，作为外部协作者收发消息，不会成为 Team Worker。

### English
**Workspace & Intercom Circle Guide**
1. **Workspace Name**: The card title and the underlying tmux session name, used to specify which intercom circle you want to join.
2. **In-Circle Nickname (/name)**: `/name` only sets your display name inside that intercom circle; it does not rename the workspace or change your session ID.
3. **Join Intercom Circle (join)**: `join` adds you to that workspace’s intercom circle to send and receive messages as an external collaborator; it does not make you a Team Worker.
