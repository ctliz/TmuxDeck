import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import {
  Terminal,
  Plus,
  RefreshCw,
  Play,
  Trash2,
  Edit2,
  Folder,
  CheckCircle2,
  XCircle,
  Search,
  LayoutGrid,
  Zap,
  Bot,
} from "lucide-react";

interface TmuxPane {
  id: String;
  command: string;
  active: boolean;
}

interface TmuxSession {
  id: string;
  name: string;
  windows_count: number;
  panes_count: number;
  attached: boolean;
  created_at: string;
  panes: TmuxPane[];
}

interface EnvStatus {
  tmux_installed: boolean;
  tmux_path: string;
  ghostty_installed: boolean;
  ghostty_path: string;
  pi_installed: boolean;
  pi_path: string;
}

export default function App() {
  const [sessions, setSessions] = useState<TmuxSession[]>([]);
  const [envStatus, setEnvStatus] = useState<EnvStatus | null>(null);
  const [search, setSearch] = useState("");
  const [loading, setLoading] = useState(false);
  const [errorMsg, setErrorMsg] = useState("");
  const [showCreateModal, setShowCreateModal] = useState(false);
  const [newSessionName, setNewSessionName] = useState("");
  const [workingDir, setWorkingDir] = useState("");
  const [renamingSession, setRenamingSession] = useState<string | null>(null);
  const [renamedName, setRenamedName] = useState("");

  const loadSessions = async () => {
    setLoading(true);
    setErrorMsg("");
    try {
      const data = await invoke<TmuxSession[]>("get_tmux_sessions");
      setSessions(data);
    } catch (err: any) {
      setErrorMsg(err?.toString() || "加载 session 失败");
    } finally {
      setLoading(false);
    }
  };

  const checkEnvironment = async () => {
    try {
      const env = await invoke<EnvStatus>("check_env");
      setEnvStatus(env);
    } catch (err) {
      console.error("检查环境失败", err);
    }
  };

  useEffect(() => {
    checkEnvironment();
    loadSessions();
    const timer = setInterval(() => {
      loadSessions();
    }, 4000);
    return () => clearInterval(timer);
  }, []);

  const handleAttach = async (sessionName: string) => {
    try {
      await invoke("attach_session", { sessionName });
    } catch (err: any) {
      alert("启动 Ghostty 失败: " + err);
    }
  };

  const handleCreate = async () => {
    if (!newSessionName.trim()) {
      alert("请输入项目名称");
      return;
    }
    setLoading(true);
    try {
      await invoke("create_4pi_session", {
        sessionName: newSessionName.trim(),
        workingDir: workingDir.trim() || null,
      });
      setShowCreateModal(false);
      setNewSessionName("");
      setWorkingDir("");
      await loadSessions();
    } catch (err: any) {
      alert("创建失败: " + err);
    } finally {
      setLoading(false);
    }
  };

  const handleKill = async (sessionName: string) => {
    if (!confirm(`确定要销毁工作区 [${sessionName}] 吗？`)) return;
    try {
      await invoke("kill_session", { sessionName });
      await loadSessions();
    } catch (err: any) {
      alert("销毁失败: " + err);
    }
  };

  const handleRename = async (oldName: string) => {
    if (!renamedName.trim() || renamedName === oldName) {
      setRenamingSession(null);
      return;
    }
    try {
      await invoke("rename_session", { oldName, newName: renamedName.trim() });
      setRenamingSession(null);
      await loadSessions();
    } catch (err: any) {
      alert("重命名失败: " + err);
    }
  };

  const filteredSessions = sessions.filter((s) =>
    s.name.toLowerCase().includes(search.toLowerCase())
  );

  return (
    <div className="flex flex-col h-screen bg-slate-950 text-slate-100 font-sans">
      {/* 顶部 App Header */}
      <header className="flex items-center justify-between px-6 py-4 border-b border-slate-800 bg-slate-900/60 backdrop-blur-md">
        <div className="flex items-center space-x-3">
          <div className="p-2 rounded-xl bg-gradient-to-tr from-cyan-500 to-blue-600 shadow-lg shadow-cyan-500/20">
            <LayoutGrid className="w-6 h-6 text-white" />
          </div>
          <div>
            <div className="flex items-center space-x-2">
              <h1 className="text-xl font-bold bg-gradient-to-r from-white via-slate-200 to-slate-400 bg-clip-text text-transparent">
                TmuxDeck
              </h1>
              <span className="px-2 py-0.5 text-xs font-semibold rounded-full bg-cyan-950 text-cyan-400 border border-cyan-800">
                v1.0
              </span>
            </div>
            <p className="text-xs text-slate-400">
              Ghostty & Tmux 4-Pi Agent 工作区控制台
            </p>
          </div>
        </div>

        {/* 环境识别指示器 */}
        <div className="hidden md:flex items-center space-x-4 px-3 py-1.5 rounded-lg bg-slate-900 border border-slate-800 text-xs">
          <div className="flex items-center space-x-1.5">
            {envStatus?.tmux_installed ? (
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
            ) : (
              <XCircle className="w-3.5 h-3.5 text-rose-400" />
            )}
            <span className="text-slate-300">Tmux</span>
          </div>
          <div className="flex items-center space-x-1.5">
            {envStatus?.ghostty_installed ? (
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
            ) : (
              <XCircle className="w-3.5 h-3.5 text-rose-400" />
            )}
            <span className="text-slate-300">Ghostty</span>
          </div>
          <div className="flex items-center space-x-1.5">
            {envStatus?.pi_installed ? (
              <CheckCircle2 className="w-3.5 h-3.5 text-emerald-400" />
            ) : (
              <XCircle className="w-3.5 h-3.5 text-amber-400" />
            )}
            <span className="text-slate-300">Pi Agent</span>
          </div>
        </div>

        {/* 顶部操作区 */}
        <div className="flex items-center space-x-3">
          <button
            onClick={loadSessions}
            disabled={loading}
            className="p-2 rounded-lg bg-slate-900 border border-slate-800 text-slate-300 hover:text-white hover:bg-slate-800 transition disabled:opacity-50"
            title="刷新列表"
          >
            <RefreshCw className={`w-4 h-4 ${loading ? "animate-spin" : ""}`} />
          </button>
          <button
            onClick={() => {
              setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
              setShowCreateModal(true);
            }}
            className="flex items-center space-x-2 px-4 py-2 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium shadow-lg shadow-cyan-500/25 transition text-sm cursor-pointer"
          >
            <Plus className="w-4 h-4" />
            <span>新建 4-Pi 工作区</span>
          </button>
        </div>
      </header>

      {/* 搜索与统计栏 */}
      <div className="flex items-center justify-between px-6 py-3 bg-slate-900/40 border-b border-slate-800/60">
        <div className="relative w-72">
          <Search className="w-4 h-4 absolute left-3 top-2.5 text-slate-500" />
          <input
            type="text"
            placeholder="搜索项目名称..."
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            className="w-full pl-9 pr-4 py-1.5 text-sm bg-slate-900 border border-slate-800 rounded-lg text-slate-200 focus:outline-none focus:border-cyan-500 transition"
          />
        </div>
        <div className="text-xs text-slate-400 flex items-center space-x-4">
          <span>共 <strong className="text-cyan-400">{sessions.length}</strong> 个项目工作区</span>
          <span>运行中: <strong className="text-emerald-400">{sessions.filter(s => s.attached).length}</strong></span>
        </div>
      </div>

      {/* 主体卡片区域 */}
      <main className="flex-1 overflow-y-auto p-6">
        {errorMsg && (
          <div className="mb-6 p-4 rounded-xl bg-rose-950/40 border border-rose-800/60 text-rose-300 text-sm">
            {errorMsg}
          </div>
        )}

        {filteredSessions.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-64 text-center">
            <div className="p-4 rounded-2xl bg-slate-900/80 border border-slate-800 mb-4">
              <Terminal className="w-10 h-10 text-slate-600" />
            </div>
            <h3 className="text-lg font-semibold text-slate-300">暂无匹配的 Tmux 工作区</h3>
            <p className="text-sm text-slate-500 max-w-sm mt-1 mb-4">
              点击右上角的“新建 4-Pi 工作区”快速创建一个分屏包含 4 个 Pi Agent 的项目卡片
            </p>
            <button
              onClick={() => {
                setNewSessionName(`project-${Math.floor(Math.random() * 900 + 100)}`);
                setShowCreateModal(true);
              }}
              className="flex items-center space-x-2 px-4 py-2 rounded-xl bg-slate-900 border border-slate-800 hover:bg-slate-800 text-cyan-400 text-sm font-medium transition"
            >
              <Plus className="w-4 h-4" />
              <span>立即新建工作区</span>
            </button>
          </div>
        ) : (
          <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4 gap-5">
            {filteredSessions.map((session) => {
              const isRenaming = renamingSession === session.name;
              return (
                <div
                  key={session.id}
                  className="flex flex-col justify-between rounded-2xl bg-slate-900/80 border border-slate-800 hover:border-cyan-500/50 transition shadow-lg group hover:shadow-cyan-500/5"
                >
                  {/* 卡片头部 */}
                  <div className="p-4 border-b border-slate-800/60">
                    <div className="flex items-center justify-between mb-2">
                      <div className="flex items-center space-x-2 min-w-0 flex-1">
                        <span
                          className={`w-2.5 h-2.5 rounded-full shrink-0 ${
                            session.attached
                              ? "bg-emerald-400 shadow-sm shadow-emerald-400/80 animate-pulse"
                              : "bg-slate-600"
                          }`}
                          title={session.attached ? "活动中 Attached" : "空闲 Idle"}
                        />
                        {isRenaming ? (
                          <input
                            type="text"
                            value={renamedName}
                            onChange={(e) => setRenamedName(e.target.value)}
                            onBlur={() => handleRename(session.name)}
                            onKeyDown={(e) => e.key === "Enter" && handleRename(session.name)}
                            autoFocus
                            className="bg-slate-950 px-2 py-0.5 border border-cyan-500 rounded text-sm text-white w-full"
                          />
                        ) : (
                          <h2 className="font-semibold text-slate-100 truncate text-base group-hover:text-cyan-300 transition">
                            {session.name}
                          </h2>
                        )}
                      </div>
                      <div className="flex items-center space-x-1 shrink-0">
                        <button
                          onClick={() => {
                            setRenamingSession(session.name);
                            setRenamedName(session.name);
                          }}
                          className="p-1 rounded text-slate-500 hover:text-slate-300 hover:bg-slate-800 opacity-0 group-hover:opacity-100 transition"
                          title="重命名"
                        >
                          <Edit2 className="w-3.5 h-3.5" />
                        </button>
                        <button
                          onClick={() => handleKill(session.name)}
                          className="p-1 rounded text-slate-500 hover:text-rose-400 hover:bg-slate-800 opacity-0 group-hover:opacity-100 transition"
                          title="销毁 Session"
                        >
                          <Trash2 className="w-3.5 h-3.5" />
                        </button>
                      </div>
                    </div>

                    <div className="flex items-center justify-between text-xs text-slate-400">
                      <span>{session.windows_count} 窗口 · {session.panes_count} 分屏</span>
                      <span>{session.created_at}</span>
                    </div>
                  </div>

                  {/* 4-Pi 平铺预览卡片示意区 */}
                  <div className="p-4 flex-1">
                    <div className="text-xs text-slate-500 mb-2 font-medium flex items-center justify-between">
                      <span>4-Pi 屏阵列布局:</span>
                      {session.panes.some(p => p.command.includes("pi")) && (
                        <span className="flex items-center space-x-1 text-cyan-400 text-[10px]">
                          <Bot className="w-3 h-3" />
                          <span>Pi Ready</span>
                        </span>
                      )}
                    </div>
                    <div className="grid grid-cols-2 gap-2 p-2 rounded-xl bg-slate-950/80 border border-slate-800/80">
                      {Array.from({ length: 4 }).map((_, idx) => {
                        const pane = session.panes[idx];
                        const cmdName = pane ? pane.command : "empty";
                        const isPi = cmdName.includes("pi");
                        return (
                          <div
                            key={idx}
                            className={`flex flex-col justify-between p-2 rounded-lg border text-[11px] h-12 transition ${
                              isPi
                                ? "bg-cyan-950/30 border-cyan-800/40 text-cyan-300"
                                : "bg-slate-900/60 border-slate-800 text-slate-400"
                            }`}
                          >
                            <div className="flex items-center justify-between">
                              <span className="font-mono text-[9px] text-slate-500">#{idx + 1}</span>
                              {isPi && <Zap className="w-2.5 h-2.5 text-cyan-400" />}
                            </div>
                            <span className="font-mono truncate font-medium">
                              {cmdName}
                            </span>
                          </div>
                        );
                      })}
                    </div>
                  </div>

                  {/* 卡片底部操作按钮 */}
                  <div className="p-3 border-t border-slate-800/60 bg-slate-900/40">
                    <button
                      onClick={() => handleAttach(session.name)}
                      className="w-full flex items-center justify-center space-x-2 py-2 px-3 rounded-xl bg-slate-800 hover:bg-gradient-to-r hover:from-cyan-600 hover:to-blue-600 text-slate-200 hover:text-white text-sm font-medium transition shadow-sm group-hover:bg-cyan-600/20 group-hover:text-cyan-300 group-hover:border group-hover:border-cyan-500/40 cursor-pointer"
                    >
                      <Play className="w-3.5 h-3.5 fill-current" />
                      <span>恢复会话 (Ghostty)</span>
                    </button>
                  </div>
                </div>
              );
            })}
          </div>
        )}
      </main>

      {/* 新建项目 Modal */}
      {showCreateModal && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-950/80 backdrop-blur-sm p-4">
          <div className="w-full max-w-md rounded-2xl bg-slate-900 border border-slate-800 shadow-2xl p-6">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-2">
                <div className="p-2 rounded-lg bg-cyan-950 border border-cyan-800 text-cyan-400">
                  <Bot className="w-5 h-5" />
                </div>
                <h3 className="text-lg font-bold text-slate-100">新建 4-Pi 项目工作区</h3>
              </div>
              <button
                onClick={() => setShowCreateModal(false)}
                className="text-slate-400 hover:text-white"
              >
                ✕
              </button>
            </div>

            <div className="space-y-4">
              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1">
                  项目/Session 名称 *
                </label>
                <input
                  type="text"
                  placeholder="例如: my-ai-project"
                  value={newSessionName}
                  onChange={(e) => setNewSessionName(e.target.value)}
                  className="w-full px-3 py-2 text-sm bg-slate-950 border border-slate-800 rounded-xl text-slate-100 focus:outline-none focus:border-cyan-500"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-400 mb-1">
                  工作目录 (可选)
                </label>
                <div className="relative">
                  <Folder className="w-4 h-4 absolute left-3 top-2.5 text-slate-500" />
                  <input
                    type="text"
                    placeholder="例如: /Users/username/Desktop/my-project"
                    value={workingDir}
                    onChange={(e) => setWorkingDir(e.target.value)}
                    className="w-full pl-9 pr-3 py-2 text-sm bg-slate-950 border border-slate-800 rounded-xl text-slate-100 focus:outline-none focus:border-cyan-500"
                  />
                </div>
              </div>

              <div className="p-3 rounded-xl bg-slate-950/60 border border-slate-800/60 text-xs text-slate-400 space-y-1">
                <p className="font-semibold text-slate-300">自动配置：</p>
                <p>• 自动创建一个平铺 (tiled) 4 分屏的 Tmux 窗口</p>
                <p>• 4 个分屏独立启动 <code className="text-cyan-400">pi</code> agent 引擎</p>
                <p>• 创建成功后自动拉起 <code className="text-cyan-400">Ghostty</code> 恢复焦点</p>
              </div>
            </div>

            <div className="flex items-center justify-end space-x-3 mt-6">
              <button
                onClick={() => setShowCreateModal(false)}
                className="px-4 py-2 rounded-xl text-sm font-medium text-slate-400 hover:text-slate-200"
              >
                取消
              </button>
              <button
                onClick={handleCreate}
                disabled={loading}
                className="flex items-center space-x-2 px-5 py-2 rounded-xl bg-gradient-to-r from-cyan-500 to-blue-600 hover:from-cyan-400 hover:to-blue-500 text-white font-medium text-sm transition shadow-lg shadow-cyan-500/20 disabled:opacity-50"
              >
                <Plus className="w-4 h-4" />
                <span>{loading ? "创建中..." : "创建并启动"}</span>
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
