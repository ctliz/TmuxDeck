import { useEffect, useRef, useState, useMemo, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, UnlistenFn } from "@tauri-apps/api/event";
import {
  ArrowLeft,
  ExternalLink,
  Radio,
  Maximize2,
  LayoutGrid,
  Rows3,
  Plus,
  ChevronDown,
  RefreshCw,
  AlertCircle,
} from "lucide-react";
import { agentDisplayName, t, tPlural, translateName } from "../i18n";
import { Environment, TmuxPane, TmuxSession } from "../types";
import { dominantAgentId, resolvePaneAgentId } from "../utils";

export interface TerminalCell {
  text: string;
  fg: string;
  bg: string;
  bold: boolean;
  italic: boolean;
  underline: boolean;
}

export interface TerminalCursor {
  x: number;
  y: number;
  visible: boolean;
}

export interface TerminalRowUpdate {
  row: number;
  cells: TerminalCell[];
}

export interface TerminalScrollbar {
  total: number;
  offset: number;
  length: number;
}

export interface TerminalFramePayload {
  terminalId: string;
  cols: number;
  rows: number;
  full: boolean;
  updates: TerminalRowUpdate[];
  cursor: TerminalCursor;
  mouseReporting: boolean;
  scrollbar: TerminalScrollbar;
}

export interface TerminalExitPayload {
  terminalId: string;
}

export interface AgentTerminalCanvasProps {
  session: TmuxSession;
  activePaneId: string;
  onSelectPane: (paneId: string) => void;
  onBack: () => void;
  env: Environment | null;
  selectedTerminal?: string;
  onOpenExternalTerminal?: (sessionName: string, termId?: string) => void;
  onAddPane?: (name: string, agentId: string, count: number) => Promise<void>;
  onSwapPane?: (
    paneIdA: string,
    paneIdB: string,
    sessionTargetA?: string,
    sessionTargetB?: string
  ) => Promise<void>;
}

interface SelectionRange {
  startX: number;
  startY: number;
  endX: number;
  endY: number;
}

export type TerminalLayoutMode = "focus" | "grid" | "list";

const FONT_SIZE = 13;
const FONT_FAMILY =
  '"JetBrains Mono", Menlo, Monaco, "PingFang SC", "Hiragino Sans GB", "Microsoft YaHei", "Noto Sans Mono CJK SC", "Courier New", monospace';
const LINE_HEIGHT_RATIO = 1.25;
const DEFAULT_BG = "#090d16";
const DEFAULT_FG = "#f1f5f9";
const CURSOR_COLOR = "#38bdf8";

function createBlankRow(cols: number): TerminalCell[] {
  const row: TerminalCell[] = [];
  for (let c = 0; c < cols; c++) {
    row.push({
      text: " ",
      fg: DEFAULT_FG,
      bg: DEFAULT_BG,
      bold: false,
      italic: false,
      underline: false,
    });
  }
  return row;
}

function createBlankGrid(cols: number, rows: number): TerminalCell[][] {
  const grid: TerminalCell[][] = [];
  for (let r = 0; r < rows; r++) {
    grid.push(createBlankRow(cols));
  }
  return grid;
}

function isDefaultBg(bg: string | undefined): boolean {
  if (!bg) return true;
  const n = bg.trim().toLowerCase();
  return (
    n === DEFAULT_BG.toLowerCase() ||
    n === "#000000" ||
    n === "#000" ||
    n === "rgb(0,0,0)" ||
    n === "rgb(0, 0, 0)" ||
    n === "rgba(0,0,0,1)" ||
    n === "rgba(0, 0, 0, 1)" ||
    n === "rgba(0,0,0,0)" ||
    n === "rgba(0, 0, 0, 0)" ||
    n === "transparent" ||
    n === "inherit"
  );
}

function isCellSelected(c: number, r: number, sel: SelectionRange | null): boolean {
  if (!sel) return false;
  let { startX, startY, endX, endY } = sel;
  if (startY > endY || (startY === endY && startX > endX)) {
    [startX, endX] = [endX, startX];
    [startY, endY] = [endY, startY];
  }

  if (r < startY || r > endY) return false;
  if (r === startY && r === endY) {
    return c >= startX && c <= endX;
  }
  if (r === startY) {
    return c >= startX;
  }
  if (r === endY) {
    return c <= endX;
  }
  return true;
}

function getNormalizedSelection(sel: SelectionRange): { startX: number; startY: number; endX: number; endY: number } {
  let { startX, startY, endX, endY } = sel;
  if (startY > endY || (startY === endY && startX > endX)) {
    return { startX: endX, startY: endY, endX: startX, endY: startY };
  }
  return { startX, startY, endX, endY };
}

interface SinglePaneCanvasProps {
  pane: TmuxPane;
  paneIndex: number;
  env: Environment | null;
  isActive: boolean;
  layoutMode: TerminalLayoutMode;
  sessionName?: string;
  onFocus: () => void;
  onMaximize?: () => void;
  onStatusChange?: (status: "connecting" | "attached" | "exited" | "error") => void;
  onOpenExternalTerminal?: (sessionName: string, termId?: string) => void;
  isDraggable?: boolean;
  onDragStart?: (e: React.DragEvent) => void;
  onDragOver?: (e: React.DragEvent) => void;
  onDragLeave?: (e: React.DragEvent) => void;
  onDrop?: (e: React.DragEvent) => void;
  onDragEnd?: (e: React.DragEvent) => void;
  isDragging?: boolean;
  isDragOver?: boolean;
}

function SinglePaneCanvas({
  pane,
  paneIndex,
  env,
  isActive,
  layoutMode,
  sessionName,
  onFocus,
  onMaximize,
  onStatusChange,
  onOpenExternalTerminal,
  isDraggable,
  onDragStart,
  onDragOver,
  onDragLeave,
  onDrop,
  onDragEnd,
  isDragging,
  isDragOver,
}: SinglePaneCanvasProps) {
  const [errorMessage, setErrorMessage] = useState<string | null>(null);
  const [retryCount, setRetryCount] = useState(0);

  const containerRef = useRef<HTMLDivElement | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement | null>(null);
  const terminalIdRef = useRef<string | null>(null);
  const gridBufferRef = useRef<TerminalCell[][]>([]);
  const prevCursorRef = useRef<TerminalCursor>({ x: 0, y: 0, visible: false });
  const mouseReportingRef = useRef<boolean>(false);
  const scrollbarRef = useRef<TerminalScrollbar | null>(null);
  const selectionRef = useRef<SelectionRange | null>(null);
  const isSelectingRef = useRef<boolean>(false);

  const isMountedRef = useRef(true);
  const isComposingRef = useRef(false);
  const isActiveRef = useRef(isActive);
  const onStatusChangeRef = useRef(onStatusChange);

  const charWidthRef = useRef<number>(7.8);
  const charHeightRef = useRef<number>(Math.round(FONT_SIZE * LINE_HEIGHT_RATIO));
  const colsRef = useRef<number>(80);
  const rowsRef = useRef<number>(24);

  const paneAgentId = resolvePaneAgentId(pane, env?.agents ?? []);
  const matchedAgent = paneAgentId
    ? env?.agents.find((a) => a.id === paneAgentId)
    : undefined;
  const paneAgentName = matchedAgent
    ? agentDisplayName(matchedAgent)
    : paneAgentId || "Agent";
  const cmdName = pane.command || "shell";

  useEffect(() => {
    isActiveRef.current = isActive;
    onStatusChangeRef.current = onStatusChange;
  }, [isActive, onStatusChange]);

  // Measure font metrics on a test canvas context
  const measureCharMetrics = useCallback(() => {
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.font = `${FONT_SIZE}px ${FONT_FAMILY}`;
    const metrics = ctx.measureText("M");
    const width = metrics.width > 0 ? metrics.width : 7.8;
    const height = Math.ceil(FONT_SIZE * LINE_HEIGHT_RATIO);
    charWidthRef.current = width;
    charHeightRef.current = height;
  }, []);

  // Draw scrollbar on canvas
  const drawScrollbar = useCallback((ctx: CanvasRenderingContext2D, displayWidth: number, displayHeight: number) => {
    const scrollbar = scrollbarRef.current;
    if (
      !scrollbar ||
      scrollbar.total <= scrollbar.length ||
      scrollbar.total <= 0 ||
      scrollbar.offset + scrollbar.length >= scrollbar.total
    ) return;

    const trackHeight = displayHeight;
    const thumbHeight = Math.max(16, Math.min(trackHeight, Math.round((scrollbar.length / scrollbar.total) * trackHeight)));
    const maxOffset = Math.max(1, scrollbar.total - scrollbar.length);
    const clampedOffset = Math.max(0, Math.min(maxOffset, scrollbar.offset));
    const thumbY = Math.round((clampedOffset / maxOffset) * (trackHeight - thumbHeight));
    const barWidth = 4;
    const barX = displayWidth - barWidth - 2;

    ctx.fillStyle = "rgba(148, 163, 184, 0.4)";
    if ("roundRect" in ctx && typeof ctx.roundRect === "function") {
      ctx.beginPath();
      ctx.roundRect(barX, thumbY, barWidth, thumbHeight, 2);
      ctx.fill();
    } else {
      ctx.fillRect(barX, thumbY, barWidth, thumbHeight);
    }
  }, []);

  // Helper to draw a single row cleanly (Pass 1 clear & custom bg, Pass 2 glyphs & underlines, Pass 3 selections)
  const drawSingleRow = useCallback((
    ctx: CanvasRenderingContext2D,
    r: number,
    cols: number,
    rows: number,
    charWidth: number,
    charHeight: number,
    displayWidth: number
  ) => {
    if (r < 0 || r >= rows) return;
    const grid = gridBufferRef.current;
    const rowCells = grid[r];
    if (!rowCells) return;

    const y = r * charHeight;

    // Clear row background to transparent so container backdrop-blur shines through
    ctx.clearRect(0, y, displayWidth, charHeight);

    const sel = selectionRef.current;

    // --- PASS 1: Draw all cell custom backgrounds (if non-default) ---
    for (let c = 0; c < cols; c++) {
      const cell = rowCells[c];
      if (!cell) continue;

      if (cell.bg && !isDefaultBg(cell.bg)) {
        ctx.fillStyle = cell.bg;
        ctx.fillRect(c * charWidth, y, charWidth, charHeight);
      }
    }

    // --- PASS 2: Draw all character glyphs and underlines ---
    for (let c = 0; c < cols; c++) {
      const cell = rowCells[c];
      if (!cell) continue;

      const x = c * charWidth;

      if (cell.text && cell.text !== " " && cell.text !== "") {
        let fontStyle = "";
        if (cell.italic) fontStyle += "italic ";
        if (cell.bold) fontStyle += "bold ";
        ctx.font = `${fontStyle}${FONT_SIZE}px ${FONT_FAMILY}`;
        ctx.fillStyle = cell.fg || DEFAULT_FG;

        const textYOffset = (charHeight - FONT_SIZE) / 2;
        ctx.fillText(cell.text, x, y + textYOffset);

        // Underline
        if (cell.underline) {
          ctx.strokeStyle = cell.fg || DEFAULT_FG;
          ctx.lineWidth = 1;
          ctx.beginPath();
          ctx.moveTo(x, y + charHeight - 1);
          ctx.lineTo(x + charWidth, y + charHeight - 1);
          ctx.stroke();
        }
      }
    }

    // --- PASS 3: Draw selection overlay ---
    for (let c = 0; c < cols; c++) {
      if (isCellSelected(c, r, sel)) {
        ctx.fillStyle = "rgba(56, 189, 248, 0.28)";
        ctx.fillRect(c * charWidth, y, charWidth, charHeight);
      }
    }
  }, []);

  // Redraw full canvas from grid buffer (used on selection drag or resize)
  const redrawCanvas = useCallback(() => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const charWidth = charWidthRef.current;
    const charHeight = charHeightRef.current;
    const cols = colsRef.current;
    const rows = rowsRef.current;
    const displayWidth = cols * charWidth;
    const displayHeight = rows * charHeight;

    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.textBaseline = "top";

    // Clear entire canvas to transparent
    ctx.clearRect(0, 0, displayWidth, displayHeight);

    for (let r = 0; r < rows; r++) {
      drawSingleRow(ctx, r, cols, rows, charWidth, charHeight, displayWidth);
    }

    // Draw active cursor
    const cursor = prevCursorRef.current;
    if (cursor && cursor.visible && cursor.x >= 0 && cursor.x < cols && cursor.y >= 0 && cursor.y < rows) {
      const curX = cursor.x * charWidth;
      const curY = cursor.y * charHeight;

      ctx.fillStyle = CURSOR_COLOR;
      ctx.globalAlpha = 0.8;
      ctx.fillRect(curX, curY, charWidth, charHeight);
      ctx.globalAlpha = 1.0;

      // Inverted character under cursor
      const rowCells = gridBufferRef.current[cursor.y];
      const curCell = rowCells ? rowCells[cursor.x] : undefined;
      if (curCell && curCell.text && curCell.text !== " " && curCell.text !== "") {
        ctx.fillStyle = DEFAULT_BG;
        ctx.font = `${FONT_SIZE}px ${FONT_FAMILY}`;
        const textYOffset = (charHeight - FONT_SIZE) / 2;
        ctx.fillText(curCell.text, curX, curY + textYOffset);
      }
    }

    drawScrollbar(ctx, displayWidth, displayHeight);
    ctx.restore();
  }, [drawScrollbar, drawSingleRow]);

  // Render dirty row patch / full frame on canvas
  const renderFrame = useCallback((frame: TerminalFramePayload) => {
    const canvas = canvasRef.current;
    if (!canvas) return;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const charWidth = charWidthRef.current;
    const charHeight = charHeightRef.current;
    const { cols, rows, full, updates, cursor, mouseReporting, scrollbar } = frame;

    const mouseModeChanged = mouseReportingRef.current !== Boolean(mouseReporting);
    mouseReportingRef.current = Boolean(mouseReporting);
    scrollbarRef.current = scrollbar || null;
    if (mouseReportingRef.current) {
      selectionRef.current = null;
      isSelectingRef.current = false;
    }

    const displayWidth = cols * charWidth;
    const displayHeight = rows * charHeight;

    // Check if canvas size needs adjustment (high-DPI)
    const targetWidth = Math.round(displayWidth * dpr);
    const targetHeight = Math.round(displayHeight * dpr);
    let sizeChanged = false;
    if (canvas.width !== targetWidth || canvas.height !== targetHeight) {
      canvas.width = targetWidth;
      canvas.height = targetHeight;
      canvas.style.width = `${displayWidth}px`;
      canvas.style.height = `${displayHeight}px`;
      sizeChanged = true;
    }

    // Manage grid buffer
    let grid = gridBufferRef.current;
    const shouldRecreateGrid =
      full ||
      sizeChanged ||
      grid.length !== rows ||
      (grid[0] && grid[0].length !== cols);

    if (shouldRecreateGrid) {
      grid = createBlankGrid(cols, rows);
      gridBufferRef.current = grid;
    }

    // Apply dirty row updates to local grid buffer
    for (const update of updates) {
      if (update.row >= 0 && update.row < rows) {
        grid[update.row] = update.cells;
      }
    }

    ctx.save();
    ctx.scale(dpr, dpr);
    ctx.textBaseline = "top";

    const prevCursor = prevCursorRef.current;

    if (full || sizeChanged || mouseModeChanged) {
      // Full frame: clear all and redraw all rows
      ctx.clearRect(0, 0, displayWidth, displayHeight);
      for (let r = 0; r < rows; r++) {
        drawSingleRow(ctx, r, cols, rows, charWidth, charHeight, displayWidth);
      }
    } else {
      // Partial dirty patch: only repaint modified rows + old/new cursor rows
      const dirtyRows = new Set<number>();
      for (const update of updates) {
        if (update.row >= 0 && update.row < rows) {
          dirtyRows.add(update.row);
        }
      }

      // Redraw old cursor row to erase previous cursor
      if (prevCursor.visible && prevCursor.y >= 0 && prevCursor.y < rows) {
        dirtyRows.add(prevCursor.y);
      }

      // Redraw new cursor row
      if (cursor && cursor.visible && cursor.y >= 0 && cursor.y < rows) {
        dirtyRows.add(cursor.y);
      }

      for (const r of dirtyRows) {
        drawSingleRow(ctx, r, cols, rows, charWidth, charHeight, displayWidth);
      }
    }

    // Draw active cursor
    if (cursor && cursor.visible && cursor.x >= 0 && cursor.x < cols && cursor.y >= 0 && cursor.y < rows) {
      const curX = cursor.x * charWidth;
      const curY = cursor.y * charHeight;

      ctx.fillStyle = CURSOR_COLOR;
      ctx.globalAlpha = 0.8;
      ctx.fillRect(curX, curY, charWidth, charHeight);
      ctx.globalAlpha = 1.0;

      // Inverted character under cursor
      const rowCells = grid[cursor.y];
      const curCell = rowCells ? rowCells[cursor.x] : undefined;
      if (curCell && curCell.text && curCell.text !== " " && curCell.text !== "") {
        ctx.fillStyle = DEFAULT_BG;
        ctx.font = `${FONT_SIZE}px ${FONT_FAMILY}`;
        const textYOffset = (charHeight - FONT_SIZE) / 2;
        ctx.fillText(curCell.text, curX, curY + textYOffset);
      }
    }

    drawScrollbar(ctx, displayWidth, displayHeight);

    prevCursorRef.current = cursor;
    ctx.restore();
  }, [drawScrollbar, drawSingleRow]);

  // Terminal connection and event listening
  useEffect(() => {
    isMountedRef.current = true;
    measureCharMetrics();

    let unlistenFrame: UnlistenFn | null = null;
    let unlistenExit: UnlistenFn | null = null;

    const terminalId = `term_${crypto.randomUUID()}`;
    terminalIdRef.current = terminalId;

    gridBufferRef.current = [];
    prevCursorRef.current = { x: 0, y: 0, visible: false };
    selectionRef.current = null;
    isSelectingRef.current = false;
    scrollbarRef.current = null;
    mouseReportingRef.current = false;

    const canvas = canvasRef.current;
    if (canvas) {
      const ctx = canvas.getContext("2d");
      if (ctx) {
        ctx.clearRect(0, 0, canvas.width, canvas.height);
      }
    }

    onStatusChangeRef.current?.("connecting");

    const init = async () => {
      try {
        setErrorMessage(null);
        const [fnFrame, fnExit] = await Promise.all([
          listen<TerminalFramePayload>("agent-terminal-frame", (event) => {
            if (!isMountedRef.current || event.payload.terminalId !== terminalId) return;
            renderFrame(event.payload);
          }),
          listen<TerminalExitPayload>("agent-terminal-exit", (event) => {
            if (!isMountedRef.current || event.payload.terminalId !== terminalId) return;
            onStatusChangeRef.current?.("exited");
          }),
        ]);

        if (!isMountedRef.current) {
          fnFrame();
          fnExit();
          return;
        }

        unlistenFrame = fnFrame;
        unlistenExit = fnExit;

        const container = containerRef.current;
        let cols = 80;
        let rows = 24;

        if (container && container.clientWidth > 0 && container.clientHeight > 0) {
          const charWidth = charWidthRef.current || 7.8;
          const charHeight = charHeightRef.current || 16;
          if (charWidth > 0 && charHeight > 0) {
            const style = window.getComputedStyle(container);
            const padLeft = parseFloat(style.paddingLeft) || 0;
            const padRight = parseFloat(style.paddingRight) || 0;
            const padTop = parseFloat(style.paddingTop) || 0;
            const padBottom = parseFloat(style.paddingBottom) || 0;
            const contentWidth = container.clientWidth - padLeft - padRight;
            const contentHeight = container.clientHeight - padTop - padBottom;
            const computedCols = Math.floor(contentWidth / charWidth);
            const computedRows = Math.floor(contentHeight / charHeight);
            if (Number.isFinite(computedCols) && computedCols >= 10) {
              cols = Math.min(500, Math.max(10, Math.floor(computedCols)));
            }
            if (Number.isFinite(computedRows) && computedRows >= 5) {
              rows = Math.min(200, Math.max(5, Math.floor(computedRows)));
            }
            colsRef.current = cols;
            rowsRef.current = rows;
          }
        }

        await invoke("open_agent_terminal", {
          terminalId,
          paneId: pane.id,
          cols,
          rows,
        });

        if (isMountedRef.current && terminalIdRef.current === terminalId) {
          setErrorMessage(null);
          onStatusChangeRef.current?.("attached");
          if (isActiveRef.current) {
            textareaRef.current?.focus();
          }
        } else {
          invoke("close_agent_terminal", { terminalId }).catch(() => {});
        }
      } catch (err) {
        console.error("open_agent_terminal failed:", err);
        const msg = err instanceof Error ? err.message : String(err);
        if (isMountedRef.current) {
          setErrorMessage(msg);
        }
        onStatusChangeRef.current?.("error");
        invoke("close_agent_terminal", { terminalId }).catch(() => {});
      }
    };

    void init();

    return () => {
      isMountedRef.current = false;
      if (unlistenFrame) unlistenFrame();
      if (unlistenExit) unlistenExit();

      const termIdToClose = terminalIdRef.current;
      if (termIdToClose) {
        terminalIdRef.current = null;
        invoke("close_agent_terminal", { terminalId: termIdToClose }).catch(() => {});
      }
    };
  }, [pane.id, retryCount, measureCharMetrics, renderFrame]);

  // Focus textarea when pane becomes active
  useEffect(() => {
    if (isActive) {
      textareaRef.current?.focus();
    }
  }, [isActive]);

  // ResizeObserver for dynamic bounds
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const observer = new ResizeObserver((entries) => {
      for (const entry of entries) {
        const { width, height } = entry.contentRect;
        const charWidth = charWidthRef.current || 7.8;
        const charHeight = charHeightRef.current || 16;
        if (width <= 0 || height <= 0 || charWidth <= 0 || charHeight <= 0) return;

        const newCols = Math.min(500, Math.max(10, Math.floor(width / charWidth)));
        const newRows = Math.min(200, Math.max(5, Math.floor(height / charHeight)));

        if (
          Number.isFinite(newCols) &&
          Number.isFinite(newRows) &&
          (newCols !== colsRef.current || newRows !== rowsRef.current)
        ) {
          colsRef.current = newCols;
          rowsRef.current = newRows;

          const currentTerminalId = terminalIdRef.current;
          if (currentTerminalId) {
            invoke("resize_agent_terminal", {
              terminalId: currentTerminalId,
              cols: newCols,
              rows: newRows,
            }).catch(() => {});
          }
        }
      }
    });

    observer.observe(container);
    return () => observer.disconnect();
  }, []);

  // Mouse & Scroll Handling
  const handleMouseDown = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const currentTerminalId = terminalIdRef.current;
    if (!currentTerminalId) return;

    onFocus();
    textareaRef.current?.focus();

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const charWidth = charWidthRef.current;
    const charHeight = charHeightRef.current;

    if (mouseReportingRef.current) {
      invoke("mouse_agent_terminal", {
        terminalId: currentTerminalId,
        action: "press",
        button: e.button,
        x,
        y,
        cellWidth: charWidth,
        cellHeight: charHeight,
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        meta: e.metaKey,
      }).catch((err) => console.error("mouse_agent_terminal press error:", err));
    } else {
      if (e.button === 0) {
        const col = Math.max(0, Math.min(colsRef.current - 1, Math.floor(x / charWidth)));
        const row = Math.max(0, Math.min(rowsRef.current - 1, Math.floor(y / charHeight)));
        selectionRef.current = { startX: col, startY: row, endX: col, endY: row };
        isSelectingRef.current = true;
        redrawCanvas();
      }
    }
  };

  const handleMouseMove = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const currentTerminalId = terminalIdRef.current;
    if (!currentTerminalId) return;

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const charWidth = charWidthRef.current;
    const charHeight = charHeightRef.current;

    if (mouseReportingRef.current) {
      if (e.buttons > 0) {
        invoke("mouse_agent_terminal", {
          terminalId: currentTerminalId,
          action: "motion",
          button: e.buttons === 1 ? 0 : e.buttons === 2 ? 2 : 1,
          x,
          y,
          cellWidth: charWidth,
          cellHeight: charHeight,
          ctrl: e.ctrlKey,
          alt: e.altKey,
          shift: e.shiftKey,
          meta: e.metaKey,
        }).catch(() => {});
      }
    } else {
      if (isSelectingRef.current && selectionRef.current) {
        const col = Math.max(0, Math.min(colsRef.current - 1, Math.floor(x / charWidth)));
        const row = Math.max(0, Math.min(rowsRef.current - 1, Math.floor(y / charHeight)));
        selectionRef.current = {
          ...selectionRef.current,
          endX: col,
          endY: row,
        };
        redrawCanvas();
      }
    }
  };

  const handleMouseUp = (e: React.MouseEvent<HTMLCanvasElement>) => {
    const currentTerminalId = terminalIdRef.current;
    if (!currentTerminalId) return;

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const charWidth = charWidthRef.current;
    const charHeight = charHeightRef.current;

    if (mouseReportingRef.current) {
      invoke("mouse_agent_terminal", {
        terminalId: currentTerminalId,
        action: "release",
        button: e.button,
        x,
        y,
        cellWidth: charWidth,
        cellHeight: charHeight,
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        meta: e.metaKey,
      }).catch((err) => console.error("mouse_agent_terminal release error:", err));
    } else {
      if (isSelectingRef.current) {
        isSelectingRef.current = false;
        if (
          selectionRef.current &&
          selectionRef.current.startX === selectionRef.current.endX &&
          selectionRef.current.startY === selectionRef.current.endY
        ) {
          selectionRef.current = null;
          redrawCanvas();
        }
      }
    }
  };

  const handleWheel = (e: React.WheelEvent<HTMLCanvasElement>) => {
    const currentTerminalId = terminalIdRef.current;
    if (!currentTerminalId) return;

    const rect = canvasRef.current?.getBoundingClientRect();
    if (!rect) return;
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const charWidth = charWidthRef.current;
    const charHeight = charHeightRef.current;

    if (mouseReportingRef.current) {
      const button = e.deltaY < 0 ? 3 : 4;
      invoke("mouse_agent_terminal", {
        terminalId: currentTerminalId,
        action: "press",
        button,
        x,
        y,
        cellWidth: charWidth,
        cellHeight: charHeight,
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        meta: e.metaKey,
      }).catch(() => {});
    } else {
      const delta = e.deltaY < 0 ? -3 : 3;
      invoke("scroll_agent_terminal", {
        terminalId: currentTerminalId,
        delta,
      }).catch((err) => {
        console.error("scroll_agent_terminal error:", err);
      });
    }
  };

  // Input & Keyboard Dispatching
  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    const currentTerminalId = terminalIdRef.current;
    if (!currentTerminalId) return;

    if (isComposingRef.current || e.nativeEvent.isComposing) {
      return;
    }

    const isCopyCombo =
      (e.ctrlKey || e.metaKey) &&
      (e.key === "c" || e.key === "C") &&
      !e.shiftKey &&
      !e.altKey;

    if (isCopyCombo && selectionRef.current && !mouseReportingRef.current) {
      e.preventDefault();
      const norm = getNormalizedSelection(selectionRef.current);
      invoke<string>("copy_agent_terminal", {
        terminalId: currentTerminalId,
        startX: norm.startX,
        startY: norm.startY,
        endX: norm.endX,
        endY: norm.endY,
        rectangle: false,
      })
        .then((text) => {
          if (text) {
            navigator.clipboard.writeText(text).catch(() => {});
          }
        })
        .catch((err) => {
          console.error("copy_agent_terminal error:", err);
        });
      return;
    }

    const isSpecialKey =
      e.key === "Enter" ||
      e.key === "Tab" ||
      e.key === "Escape" ||
      e.key === "Backspace" ||
      e.key === "Delete" ||
      e.key.startsWith("Arrow") ||
      e.key === "Home" ||
      e.key === "End" ||
      e.key === "PageUp" ||
      e.key === "PageDown" ||
      /^F\d{1,2}$/.test(e.key);

    const hasModifier = e.ctrlKey || e.altKey || e.metaKey;

    if (isSpecialKey || hasModifier) {
      e.preventDefault();
      invoke("key_agent_terminal", {
        terminalId: currentTerminalId,
        code: e.code,
        key: e.key,
        ctrl: e.ctrlKey,
        alt: e.altKey,
        shift: e.shiftKey,
        meta: e.metaKey,
      }).catch((err) => {
        console.error("key_agent_terminal error:", err);
      });
    }
  };

  const handleInput = (e: React.FormEvent<HTMLTextAreaElement>) => {
    const currentTerminalId = terminalIdRef.current;
    if (!currentTerminalId) return;

    if (isComposingRef.current) return;

    const val = e.currentTarget.value;
    if (val) {
      invoke("write_agent_terminal", {
        terminalId: currentTerminalId,
        data: val,
      }).catch((err) => {
        console.error("write_agent_terminal error:", err);
      });
      e.currentTarget.value = "";
    }
  };

  const handleCompositionStart = () => {
    isComposingRef.current = true;
  };

  const handleCompositionEnd = (e: React.CompositionEvent<HTMLTextAreaElement>) => {
    isComposingRef.current = false;
    const currentTerminalId = terminalIdRef.current;
    if (!currentTerminalId) return;

    const data = e.data || e.currentTarget.value;
    if (data) {
      invoke("write_agent_terminal", {
        terminalId: currentTerminalId,
        data,
      }).catch((err) => {
        console.error("write_agent_terminal error:", err);
      });
      e.currentTarget.value = "";
    }
  };

  const handlePaste = (e: React.ClipboardEvent<HTMLTextAreaElement>) => {
    e.preventDefault();
    const currentTerminalId = terminalIdRef.current;
    if (!currentTerminalId) return;

    const text = e.clipboardData.getData("text");
    if (text) {
      invoke("paste_agent_terminal", {
        terminalId: currentTerminalId,
        data: text,
      }).catch((err) => {
        console.error("paste_agent_terminal error on paste:", err);
      });
    }
  };

  const isFocusLayout = layoutMode === "focus";

  return (
    <div
      onClick={() => {
        onFocus();
        textareaRef.current?.focus();
      }}
      className={`flex flex-col h-full w-full rounded-2xl transition-all duration-200 overflow-hidden relative ${
        isFocusLayout
          ? "border border-white/10 bg-[#090d16]/80 backdrop-blur-2xl shadow-2xl shadow-black/80"
          : isActive
          ? "border border-cyan-500/60 ring-1 ring-cyan-500/30 bg-[#090d16]/80 backdrop-blur-2xl shadow-xl shadow-cyan-950/20"
          : "border border-white/10 hover:border-white/20 bg-[#090d16]/70 backdrop-blur-xl shadow-lg shadow-black/60"
      }`}
    >
      {/* Mini Pane Titlebar for Grid & List modes */}
      {!isFocusLayout && (
        <div
          draggable={isDraggable}
          onDragStart={onDragStart}
          onDragOver={onDragOver}
          onDragLeave={onDragLeave}
          onDrop={onDrop}
          onDragEnd={onDragEnd}
          className={`flex items-center justify-between px-3.5 py-1.5 bg-black/40 border-b border-white/10 select-none shrink-0 transition-colors ${
            isDraggable ? "cursor-grab active:cursor-grabbing" : ""
          } ${
            isDragging
              ? "opacity-30 border-cyan-400 border-dashed"
              : isDragOver
              ? "bg-cyan-500/20"
              : ""
          }`}
        >
          <div className="flex items-center space-x-2 min-w-0 pointer-events-none">
            {isDraggable && (
              <span className="text-[10px] text-slate-500">⠿</span>
            )}
            <span className="font-mono text-[10px] text-slate-400 font-semibold shrink-0">
              {pane.slot ? `Slot ${pane.slot}` : `#${paneIndex + 1}`}
            </span>
            <span className="text-xs text-slate-600">·</span>
            <span className="font-medium text-xs text-cyan-300 truncate">
              {paneAgentName}
            </span>
            <span className="text-cyan-400/80 bg-cyan-950/50 border border-cyan-500/30 px-1.5 py-0.5 rounded text-[9px] font-mono truncate max-w-[100px]">
              {cmdName}
            </span>
          </div>

          <div className="flex items-center space-x-1.5 shrink-0 pointer-events-auto">
            {onMaximize && (
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  onMaximize();
                }}
                className="p-1 rounded-md text-slate-400 hover:text-cyan-300 hover:bg-white/10 transition cursor-pointer"
                title={t("terminal.maximize")}
              >
                <Maximize2 className="w-3 h-3" />
              </button>
            )}
          </div>
        </div>
      )}

      {/* Canvas Terminal Surface */}
      <main
        ref={containerRef}
        className="flex-1 w-full h-full p-2.5 overflow-hidden relative cursor-text select-none min-h-0"
      >
        <canvas
          ref={canvasRef}
          className="block"
          onMouseDown={handleMouseDown}
          onMouseMove={handleMouseMove}
          onMouseUp={handleMouseUp}
          onWheel={handleWheel}
          onContextMenu={(event) => event.preventDefault()}
        />

        {/* Hidden focusable textarea for IME, paste, and text entry */}
        <textarea
          ref={textareaRef}
          aria-label="Agent Terminal Input"
          autoComplete="off"
          autoCapitalize="off"
          autoCorrect="off"
          spellCheck="false"
          tabIndex={0}
          className="absolute inset-0 opacity-0 pointer-events-none resize-none overflow-hidden w-full h-full p-0 m-0 border-0 outline-none"
          onKeyDown={handleKeyDown}
          onInput={handleInput}
          onCompositionStart={handleCompositionStart}
          onCompositionEnd={handleCompositionEnd}
          onPaste={handlePaste}
        />

        {/* Error / Disconnection Overlay */}
        {errorMessage && (
          <div className="absolute inset-0 z-20 flex flex-col items-center justify-center p-6 bg-[#090d16]/90 backdrop-blur-md text-center select-text">
            <div className="w-12 h-12 rounded-2xl bg-rose-500/10 border border-rose-500/30 flex items-center justify-center mb-3 text-rose-400 shadow-lg shadow-rose-950/30">
              <AlertCircle className="w-6 h-6" />
            </div>
            <h3 className="text-sm font-semibold text-slate-200 mb-1">
              {t("terminal.failedToOpen")}
            </h3>
            <p className="text-xs text-rose-300/90 font-mono max-w-md break-all bg-rose-950/40 border border-rose-900/50 rounded-lg px-3 py-1.5 my-2">
              {errorMessage}
            </p>
            <div className="flex items-center space-x-2.5 mt-3 pointer-events-auto">
              <button
                type="button"
                onClick={(e) => {
                  e.stopPropagation();
                  setRetryCount((c) => c + 1);
                }}
                className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-xl bg-cyan-500 hover:bg-cyan-400 text-slate-950 font-medium text-xs shadow-lg shadow-cyan-500/20 transition cursor-pointer"
              >
                <RefreshCw className="w-3.5 h-3.5" />
                <span>{t("terminal.retry")}</span>
              </button>
              {onOpenExternalTerminal && sessionName && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onOpenExternalTerminal(sessionName);
                  }}
                  className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-xl bg-white/10 hover:bg-white/15 border border-white/10 text-slate-200 font-medium text-xs transition cursor-pointer"
                >
                  <ExternalLink className="w-3.5 h-3.5" />
                  <span>{t("terminal.openFallback")}</span>
                </button>
              )}
            </div>
          </div>
        )}
      </main>
    </div>
  );
}

export function AgentTerminalCanvas({
  session,
  activePaneId,
  onSelectPane,
  onBack,
  env,
  selectedTerminal,
  onOpenExternalTerminal,
  onAddPane,
  onSwapPane,
}: AgentTerminalCanvasProps) {
  const isMac = typeof navigator !== "undefined" && navigator.userAgent.includes("Macintosh");
  const [termStatus, setTermStatus] = useState<"connecting" | "attached" | "exited" | "error">("connecting");
  const [layoutMode, setLayoutMode] = useState<TerminalLayoutMode>("focus");

  // Add-pane state
  const [showAddPaneMenu, setShowAddPaneMenu] = useState(false);
  const [addPaneCount, setAddPaneCount] = useState(1);
  const [addingPanes, setAddingPanes] = useState(false);

  // Tab/Pane drag & drop swap state
  const [draggingPaneId, setDraggingPaneId] = useState<string | null>(null);
  const [dragOverPaneId, setDragOverPaneId] = useState<string | null>(null);
  const isPaneDraggingRef = useRef(false);

  const handlePaneDragStart = (e: React.DragEvent, pane: TmuxPane) => {
    if (session.panes.length <= 1) return;
    e.stopPropagation();
    isPaneDraggingRef.current = true;
    setDraggingPaneId(pane.id);
    e.dataTransfer.setData(
      "application/x-tmuxdeck-pane",
      JSON.stringify({
        sessionId: session.id,
        paneId: pane.id,
        sessionTarget: pane.session_target,
      })
    );
    e.dataTransfer.effectAllowed = "move";
  };

  const handlePaneDragOver = (e: React.DragEvent, pane: TmuxPane) => {
    if (!isPaneDraggingRef.current || session.panes.length <= 1) return;
    if (draggingPaneId && draggingPaneId !== pane.id) {
      e.stopPropagation();
      e.preventDefault();
      e.dataTransfer.dropEffect = "move";
      setDragOverPaneId(pane.id);
    }
  };

  const handlePaneDragLeave = (e: React.DragEvent, pane: TmuxPane) => {
    e.stopPropagation();
    if (dragOverPaneId === pane.id) {
      setDragOverPaneId(null);
    }
  };

  const handlePaneDrop = (e: React.DragEvent, targetPane: TmuxPane) => {
    e.stopPropagation();
    e.preventDefault();
    setDragOverPaneId(null);
    setDraggingPaneId(null);
    isPaneDraggingRef.current = false;

    const rawData = e.dataTransfer.getData("application/x-tmuxdeck-pane");
    if (!rawData || !onSwapPane) return;

    try {
      const data = JSON.parse(rawData);
      if (data.paneId !== targetPane.id) {
        onSwapPane(
          data.paneId,
          targetPane.id,
          data.sessionTarget,
          targetPane.session_target
        );
      }
    } catch (err) {
      console.error("Pane drag drop swap error", err);
    }
  };

  const handlePaneDragEnd = (e: React.DragEvent) => {
    e.stopPropagation();
    setDraggingPaneId(null);
    setDragOverPaneId(null);
    isPaneDraggingRef.current = false;
  };

  const runningAgentIds = session.panes
    .map((pane) => resolvePaneAgentId(pane, env?.agents ?? []))
    .filter((agentId): agentId is string => Boolean(agentId));
  const recommendedAddAgentId = dominantAgentId(runningAgentIds);

  const runAddPane = async (agentId: string) => {
    if (!onAddPane) return;
    const count = addPaneCount;
    setShowAddPaneMenu(false);
    setAddPaneCount(1);
    setAddingPanes(true);
    try {
      await onAddPane(session.name, agentId, count);
    } finally {
      setAddingPanes(false);
    }
  };

  const activePane = useMemo(() => {
    return session.panes.find((p) => p.id === activePaneId) || session.panes[0];
  }, [session.panes, activePaneId]);

  if (!activePane) {
    return (
      <div className="td-canvas flex flex-col h-screen text-slate-100 font-sans select-none overflow-hidden items-center justify-center p-6">
        <div className="p-8 rounded-2xl bg-slate-900/80 border border-white/10 flex flex-col items-center text-center max-w-md">
          <p className="text-slate-300 text-sm font-medium mb-4">No active terminal panes found in session.</p>
          <button
            type="button"
            onClick={onBack}
            className="flex items-center space-x-1.5 px-4 py-2 rounded-xl bg-white/10 hover:bg-white/15 border border-white/15 text-slate-200 hover:text-white transition cursor-pointer text-xs font-medium"
          >
            <ArrowLeft className="w-4 h-4" />
            <span>{t("terminal.back")}</span>
          </button>
        </div>
      </div>
    );
  }

  const activePaneAgentId = activePane
    ? resolvePaneAgentId(activePane, env?.agents ?? [])
    : undefined;
  const activeAgentTool = activePaneAgentId
    ? env?.agents.find((a) => a.id === activePaneAgentId)
    : undefined;
  const activeAgentName = activeAgentTool
    ? agentDisplayName(activeAgentTool)
    : activePaneAgentId || "Agent";

  const activeTermId =
    session.terminal_id ?? (session.native_split ? "ghostty" : selectedTerminal || "ghostty");
  const matchedTerm = env?.terminals.find((term) => term.id === activeTermId);
  const termName = matchedTerm ? translateName(matchedTerm.name) : activeTermId;

  const totalPanes = session.panes.length;

  return (
    <div className="td-canvas flex flex-col h-screen text-slate-100 font-sans select-none overflow-hidden">
      {/* Header Bar */}
      <header
        data-tauri-drag-region
        className={`relative z-30 flex items-center justify-between py-3 pr-6 bg-slate-900/60 backdrop-blur-2xl border-b border-white/10 shrink-0 ${
          isMac ? "pl-20" : "pl-6"
        }`}
      >
        <div className="flex items-center space-x-3 min-w-0">
          <button
            type="button"
            onClick={onBack}
            className="flex items-center space-x-1.5 px-3.5 py-1.5 rounded-full bg-white/10 hover:bg-white/15 border border-white/15 text-slate-200 hover:text-white transition shadow-sm cursor-pointer text-xs font-medium"
            title={t("terminal.back")}
          >
            <ArrowLeft className="w-3.5 h-3.5" />
            <span>{t("terminal.back")}</span>
          </button>

          <div className="h-4 w-px bg-white/15 mx-1" />

          <div className="flex items-center space-x-2 min-w-0">
            <h1 className="text-sm font-semibold text-slate-100 truncate tracking-tight">
              {session.name}
            </h1>
            {layoutMode === "focus" && (
              <>
                <span className="text-xs text-white/30">·</span>
                <span className="text-xs text-cyan-300 font-medium px-2 py-0.5 rounded-full bg-cyan-950/50 border border-cyan-500/30 truncate">
                  {activeAgentName}
                </span>
              </>
            )}
          </div>

          {/* Pane Switcher Tabs (Shown in Focus mode when session has multiple panes) */}
          {layoutMode === "focus" && session.panes.length > 1 && (
            <div className="flex items-center space-x-1 bg-black/40 p-0.5 rounded-xl border border-white/10 ml-2">
              {session.panes.map((pane, idx) => {
                const isSelected = pane.id === activePaneId;
                const isDragging = draggingPaneId === pane.id;
                const isDragOver = dragOverPaneId === pane.id;
                const paneAgent = resolvePaneAgentId(pane, env?.agents ?? []);
                const label = pane.slot ? `Slot ${pane.slot}` : `#${idx + 1}`;
                return (
                  <button
                    key={pane.id}
                    type="button"
                    draggable={session.panes.length > 1}
                    onDragStart={(e) => handlePaneDragStart(e, pane)}
                    onDragOver={(e) => handlePaneDragOver(e, pane)}
                    onDragLeave={(e) => handlePaneDragLeave(e, pane)}
                    onDrop={(e) => handlePaneDrop(e, pane)}
                    onDragEnd={handlePaneDragEnd}
                    onClick={() => onSelectPane(pane.id)}
                    className={`px-2.5 py-1 rounded-lg text-xs font-medium transition cursor-pointer flex items-center space-x-1 ${
                      isDragging
                        ? "opacity-30 border border-dashed border-cyan-400 scale-95"
                        : isDragOver
                        ? "bg-cyan-500/30 border border-cyan-400 scale-105"
                        : isSelected
                        ? "bg-gradient-to-r from-cyan-500 to-blue-500 text-slate-950 font-bold shadow-md shadow-cyan-500/20"
                        : "text-slate-400 hover:text-slate-200 hover:bg-white/5"
                    }`}
                  >
                    <span className="cursor-grab active:cursor-grabbing text-[9px] opacity-40 hover:opacity-100 mr-0.5">
                      ⠿
                    </span>
                    <span>{label}</span>
                    {paneAgent && <span className="opacity-75">· {paneAgent}</span>}
                  </button>
                );
              })}
            </div>
          )}

          {/* Add Pane Dropdown */}
          {onAddPane && (
            <div className="relative flex items-center ml-1">
              <button
                type="button"
                disabled={addingPanes}
                onClick={() => setShowAddPaneMenu((open) => !open)}
                className={`px-2.5 py-1 rounded-xl border border-white/10 text-xs transition-all duration-200 cursor-pointer flex items-center space-x-1 disabled:opacity-50 disabled:cursor-not-allowed ${
                  showAddPaneMenu
                    ? "bg-white/15 text-cyan-300 border-cyan-500/40"
                    : "bg-white/5 hover:bg-white/10 text-slate-300 hover:text-cyan-300"
                }`}
                title={t("card.addPaneChoose")}
                aria-haspopup="menu"
                aria-expanded={showAddPaneMenu}
              >
                <Plus className="w-3.5 h-3.5 text-cyan-400" />
                <span>{addingPanes ? t("card.addPaneBusy") : t("card.addPane")}</span>
                {!addingPanes && <ChevronDown className="w-3 h-3 opacity-70" />}
              </button>

              {showAddPaneMenu && (
                <>
                  <div
                    className="fixed inset-0 z-40"
                    onClick={() => setShowAddPaneMenu(false)}
                  />
                  <div
                    role="menu"
                    className="absolute z-50 top-full left-0 mt-1 min-w-[12rem] py-1.5 rounded-2xl bg-slate-900/95 backdrop-blur-2xl border border-white/15 shadow-2xl shadow-black/80"
                  >
                    <div className="px-3 py-1 text-[9px] uppercase tracking-wide text-slate-400 font-semibold">
                      {t("card.addPaneChoose")}
                    </div>

                    <div className="flex items-center gap-1.5 px-3 py-1.5 border-b border-white/5 mb-1">
                      <span className="text-[10px] uppercase tracking-wide text-slate-400 shrink-0">
                        {t("card.addPaneCount")}
                      </span>
                      {[1, 2, 4].map((n) => (
                        <button
                          key={n}
                          type="button"
                          aria-pressed={addPaneCount === n}
                          onClick={() => setAddPaneCount(n)}
                          className={`shrink-0 w-6 py-0.5 rounded-lg text-xs font-medium tabular-nums transition cursor-pointer ${
                            addPaneCount === n
                              ? "bg-cyan-500 text-slate-950 font-bold shadow-sm"
                              : "bg-white/5 hover:bg-white/15 text-slate-300"
                          }`}
                        >
                          {n}
                        </button>
                      ))}
                    </div>

                    <div className="max-h-56 overflow-y-auto px-1 space-y-0.5">
                      {env?.agents.map((agent) => {
                        const isRecommended = agent.id === recommendedAddAgentId;
                        return (
                          <button
                            key={agent.id}
                            type="button"
                            role="menuitem"
                            onClick={() => void runAddPane(agent.id)}
                            title={tPlural("card.addPaneWith", addPaneCount, {
                              agent: agentDisplayName(agent),
                            })}
                            className={`w-full flex items-center justify-between space-x-2 text-left px-2.5 py-1.5 rounded-lg text-xs transition cursor-pointer hover:bg-white/10 ${
                              isRecommended
                                ? "text-cyan-300 bg-cyan-500/10"
                                : "text-slate-300"
                            }`}
                          >
                            <span className="truncate">
                              {agentDisplayName(agent)}
                            </span>
                            {isRecommended && (
                              <span className="shrink-0 px-1.5 py-0.5 rounded bg-cyan-500/20 border border-cyan-500/30 text-[9px] uppercase tracking-wide text-cyan-300 font-semibold">
                                {t("card.addPaneRecommended")}
                              </span>
                            )}
                          </button>
                        );
                      })}
                    </div>
                  </div>
                </>
              )}
            </div>
          )}
        </div>

        {/* Layout Switcher (Focus / List sidebar / Grid) */}
        {totalPanes > 1 && (
          <div className="flex items-center bg-black/40 p-0.5 rounded-xl border border-white/10 shadow-inner">
            <button
              type="button"
              onClick={() => setLayoutMode("focus")}
              title={t("terminal.layout.focus")}
              className={`flex items-center justify-center p-1.5 rounded-lg transition cursor-pointer ${
                layoutMode === "focus"
                  ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/40 shadow-sm"
                  : "text-slate-400 hover:text-slate-200 hover:bg-white/5 border border-transparent"
              }`}
            >
              <Maximize2 className="w-3.5 h-3.5" />
            </button>
            <button
              type="button"
              onClick={() => setLayoutMode("list")}
              title={t("terminal.layout.list")}
              className={`flex items-center justify-center p-1.5 rounded-lg transition cursor-pointer ${
                layoutMode === "list"
                  ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/40 shadow-sm"
                  : "text-slate-400 hover:text-slate-200 hover:bg-white/5 border border-transparent"
              }`}
            >
              <Rows3 className="w-3.5 h-3.5" />
            </button>
            {session.native_split && (
              <button
                type="button"
                onClick={() => setLayoutMode("grid")}
                title={t("terminal.layout.grid")}
                className={`flex items-center justify-center p-1.5 rounded-lg transition cursor-pointer ${
                  layoutMode === "grid"
                    ? "bg-cyan-500/20 text-cyan-300 border border-cyan-500/40 shadow-sm"
                    : "text-slate-400 hover:text-slate-200 hover:bg-white/5 border border-transparent"
                }`}
              >
                <LayoutGrid className="w-3.5 h-3.5" />
              </button>
            )}
          </div>
        )}

        {/* Right: Status and External Launch */}
        <div className="flex items-center space-x-3 shrink-0">
          <div
            className={`flex items-center space-x-1.5 px-3 py-1 rounded-full border text-xs font-medium ${
              termStatus === "attached"
                ? "bg-emerald-500/10 text-emerald-300 border-emerald-500/30 shadow-sm shadow-emerald-500/10"
                : termStatus === "connecting"
                ? "bg-amber-500/10 text-amber-300 border-amber-500/30 animate-pulse shadow-sm shadow-amber-500/10"
                : termStatus === "exited"
                ? "bg-slate-500/10 text-slate-300 border-slate-500/30"
                : "bg-rose-500/10 text-rose-300 border-rose-500/30 shadow-sm shadow-rose-500/10"
            }`}
          >
            <Radio
              className={`w-3 h-3 ${
                termStatus === "attached"
                  ? "text-emerald-400"
                  : termStatus === "connecting"
                  ? "text-amber-400 animate-spin"
                  : "text-slate-400"
              }`}
            />
            <span>
              {termStatus === "attached"
                ? t("terminal.status.connected")
                : termStatus === "connecting"
                ? t("terminal.status.connecting")
                : termStatus === "exited"
                ? t("terminal.status.exited")
                : t("terminal.status.disconnected")}
            </span>
          </div>

          {onOpenExternalTerminal && (
            <button
              type="button"
              onClick={() => onOpenExternalTerminal(session.name, activeTermId)}
              className="p-1.5 rounded-full bg-white/5 hover:bg-white/10 border border-white/10 text-slate-400 hover:text-slate-200 transition cursor-pointer flex items-center space-x-1 text-xs shadow-sm"
              title={`${t("terminal.openFallback")} (${termName})`}
            >
              <ExternalLink className="w-3.5 h-3.5" />
            </button>
          )}
        </div>
      </header>

      {/* Main Terminal Viewport */}
      <div className="flex-1 p-3 overflow-hidden min-h-0">
        {layoutMode === "focus" ? (
          <SinglePaneCanvas
            key={activePane.id}
            pane={activePane}
            paneIndex={session.panes.findIndex((p) => p.id === activePane.id)}
            env={env}
            isActive={true}
            layoutMode="focus"
            sessionName={session.name}
            onFocus={() => {}}
            onStatusChange={setTermStatus}
            onOpenExternalTerminal={onOpenExternalTerminal}
          />
        ) : layoutMode === "list" ? (
          <div className="flex h-full w-full gap-3 overflow-hidden">
            {/* Left Sidebar: Conversations & Panes List */}
            <aside className="w-64 md:w-72 flex-shrink-0 flex flex-col rounded-2xl bg-[#090d16]/75 backdrop-blur-2xl border border-white/10 p-2.5 overflow-hidden shadow-xl">
              <div className="px-2 py-1.5 mb-1 flex items-center justify-between border-b border-white/5 select-none">
                <span className="text-[11px] font-semibold text-slate-300 uppercase tracking-wider">
                  {t("terminal.listTitle")}
                </span>
                <span className="text-[10px] text-slate-500 font-mono">
                  {session.panes.length} {session.panes.length === 1 ? "pane" : "panes"}
                </span>
              </div>

              <div className="flex-1 overflow-y-auto space-y-1.5 pr-0.5 pt-1">
                {session.panes.map((pane, idx) => {
                  const isSelected = pane.id === activePaneId;
                  const isDragging = draggingPaneId === pane.id;
                  const isDragOver = dragOverPaneId === pane.id;
                  const paneAgentId = resolvePaneAgentId(pane, env?.agents ?? []);
                  const matchedAgent = paneAgentId
                    ? env?.agents.find((a) => a.id === paneAgentId)
                    : undefined;
                  const paneAgentName = matchedAgent
                    ? agentDisplayName(matchedAgent)
                    : paneAgentId || "Agent";
                  const cmdName = pane.command || "shell";
                  const label = pane.slot ? `Slot ${pane.slot}` : `#${idx + 1}`;

                  return (
                    <button
                      key={pane.id}
                      type="button"
                      draggable={session.panes.length > 1}
                      onDragStart={(e) => handlePaneDragStart(e, pane)}
                      onDragOver={(e) => handlePaneDragOver(e, pane)}
                      onDragLeave={(e) => handlePaneDragLeave(e, pane)}
                      onDrop={(e) => handlePaneDrop(e, pane)}
                      onDragEnd={handlePaneDragEnd}
                      onClick={() => onSelectPane(pane.id)}
                      className={`w-full text-left p-2.5 rounded-xl border transition-all duration-200 cursor-pointer flex flex-col space-y-1.5 ${
                        isDragging
                          ? "opacity-30 border-cyan-500 border-dashed scale-95"
                          : isDragOver
                          ? "bg-cyan-500/25 border-cyan-400 shadow-md scale-[1.02]"
                          : isSelected
                          ? "bg-cyan-500/15 border-cyan-500/50 shadow-md shadow-cyan-950/40 text-cyan-200"
                          : "bg-white/5 border-white/5 hover:bg-white/10 hover:border-white/15 text-slate-300"
                      }`}
                    >
                      <div className="flex items-center justify-between w-full">
                        <div className="flex items-center space-x-1.5 min-w-0">
                          <span className="cursor-grab active:cursor-grabbing text-[10px] text-slate-500 mr-0.5">
                            ⠿
                          </span>
                          <span
                            className={`font-mono text-[10px] font-semibold ${
                              isSelected ? "text-cyan-300" : "text-slate-400"
                            }`}
                          >
                            {label}
                          </span>
                          <span className="text-[10px] text-slate-600">·</span>
                          <span className="font-medium text-xs truncate">
                            {paneAgentName}
                          </span>
                        </div>
                        {isSelected && (
                          <span className="w-1.5 h-1.5 rounded-full bg-cyan-400 shrink-0 shadow-sm shadow-cyan-400" />
                        )}
                      </div>

                      <div className="flex items-center justify-between text-[9px] font-mono text-slate-400 pt-0.5">
                        <span className="text-cyan-300/80 bg-cyan-950/50 border border-cyan-500/30 px-1.5 py-0.5 rounded truncate max-w-[90px]">
                          {cmdName}
                        </span>
                        <span
                          className="text-slate-500 truncate max-w-[100px]"
                          title={pane.session_target || pane.id}
                        >
                          {pane.id || pane.session_target}
                        </span>
                      </div>
                    </button>
                  );
                })}
              </div>
            </aside>

            {/* Right Main Area: Terminal & Input Area */}
            <main className="flex-1 h-full min-w-0 flex flex-col">
              <SinglePaneCanvas
                key={activePane.id}
                pane={activePane}
                paneIndex={session.panes.findIndex((p) => p.id === activePane.id)}
                env={env}
                isActive={true}
                layoutMode="focus"
                sessionName={session.name}
                onFocus={() => {}}
                onStatusChange={setTermStatus}
                onOpenExternalTerminal={onOpenExternalTerminal}
              />
            </main>
          </div>
        ) : layoutMode === "grid" && session.native_split ? (
          <div
            className={`grid gap-3 h-full w-full ${
              totalPanes === 1
                ? "grid-cols-1 grid-rows-1"
                : totalPanes === 2
                ? "grid-cols-1 md:grid-cols-2 grid-rows-1"
                : totalPanes <= 4
                ? "grid-cols-2 grid-rows-2"
                : "grid-cols-3 grid-rows-2"
            }`}
          >
            {session.panes.map((pane, idx) => (
              <SinglePaneCanvas
                key={pane.id}
                pane={pane}
                paneIndex={idx}
                env={env}
                isActive={pane.id === activePaneId}
                layoutMode="grid"
                sessionName={session.name}
                onFocus={() => onSelectPane(pane.id)}
                onMaximize={() => {
                  onSelectPane(pane.id);
                  setLayoutMode("focus");
                }}
                onStatusChange={(status) => {
                  if (pane.id === activePaneId) {
                    setTermStatus(status);
                  }
                }}
                onOpenExternalTerminal={onOpenExternalTerminal}
                isDraggable={session.panes.length > 1}
                isDragging={draggingPaneId === pane.id}
                isDragOver={dragOverPaneId === pane.id}
                onDragStart={(e) => handlePaneDragStart(e, pane)}
                onDragOver={(e) => handlePaneDragOver(e, pane)}
                onDragLeave={(e) => handlePaneDragLeave(e, pane)}
                onDrop={(e) => handlePaneDrop(e, pane)}
                onDragEnd={handlePaneDragEnd}
              />
            ))}
          </div>
        ) : (
          <SinglePaneCanvas
            key={activePane.id}
            pane={activePane}
            paneIndex={session.panes.findIndex((p) => p.id === activePane.id)}
            env={env}
            isActive={true}
            layoutMode="focus"
            sessionName={session.name}
            onFocus={() => {}}
            onStatusChange={setTermStatus}
            onOpenExternalTerminal={onOpenExternalTerminal}
          />
        )}
      </div>
    </div>
  );
}
