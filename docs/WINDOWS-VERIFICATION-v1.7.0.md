# TmuxDeck v1.7.0 Windows on-device acceptance checklist

> Purpose: verify the Windows (WSL bridging) branch works on a real machine.
> Host: `tsiji@192.168.1.17` (OpenSSH:22, see server-deploy skill "Windows host access")
> Package under test: `TmuxDeck_1.7.0_x64-setup.exe` / `TmuxDeck_1.7.0_x64_en-US.msi` (GitHub v1.7.0 release)
> Prerequisites: WSL + default distro has tmux installed; at least one agent inside WSL (any of claude/pi/codex)
> References: acceptance criteria in docs/PRD-v1.3-windows.md, PRD-v1.11 §2.5

## A. Environment pre-check (executable over SSH)

- [x] A1 `wsl.exe -- tmux -V` outputs a version → **PASS** (tmux 3.4, tested 2026-08-10)
- [x] A2 `wsl.exe -- which <agent-bin>` finds at least one agent → **PASS** (codex, opencode; path /mnt/c/Users/80763/AppData/Roaming/npm/)
- [x] A3 Terminal shells exist: `wt.exe` / `cmd.exe` / `powershell.exe` → **PASS** (all three present)

## B. Installation (SSH silent install + file checks)

- [ ] B1 `.\TmuxDeck_1.7.0_x64-setup.exe /S` silent install with no errors
- [ ] B2 Install artifact exists: `%LOCALAPPDATA%\Programs\TmuxDeck\TmuxDeck.exe`

## C. tmux bridging functions (SSH-direct verification; bridge correctness independent of the app)

- [ ] C1 Create: `wsl.exe -- tmux new-session -d -s win-test -c /mnt/c/Users/tsiji`
- [ ] C2 Split: split successively to 4 panes + `select-layout tiled`, `list-panes -s -t win-test | wc -l` = 4
- [ ] C3 List: `wsl.exe -- tmux list-sessions` includes win-test
- [ ] C4 Open semantics: `wsl.exe -- tmux attach-session -t win-test` enters successfully (attach then detach)
- [ ] C5 Destroy: `wsl.exe -- tmux kill-session -t win-test` leaves the list empty
- [ ] C6 Path conversion: `wsl.exe wslpath -u 'C:\Users\tsiji'` = `/mnt/c/Users/tsiji`

## D. In-app verification (GUI, requires operating the Windows machine or RDP cooperation)

- [ ] D1 Launch app: empty card grid with no tmux server, no red error bar, no raw English/temp paths
- [ ] D2 New-workspace dialog: terminal row shows installed shells (wt/cmd/powershell); agent row shows agents detected inside WSL
- [ ] D3 Create 4-way split: `wsl.exe -- tmux list-panes -s -t <name> | wc -l` = 4, each pane runs the selected agent
- [ ] D4 Folder picker: after picking a `C:\...` path, the input shows the converted `/mnt/c/...`
- [ ] D5 Card click opens: terminal attaches successfully
- [ ] D6 Click the card 5 times: terminal window count stays the same (AppActivate focuses; v1.11 duplicate-open prevention)
- [ ] D7 Config persistence: `%APPDATA%\tmuxdeck\config.json` generated, defaults carried over after restart
- [ ] D8 WSL-missing guide: if WSL is not installed, the guide page shows `wsl --install` + `sudo apt install tmux` and is copyable (if WSL can't be uninstalled to test, fall back to code review)

## Division of execution

- A / B / C: fully executable over SSH
- D: needs GUI operations on the Windows machine (local user or RDP); SSH can partially verify (e.g. D7 config file, the tmux-side assertions in D3)

## Recording requirements

After execution, report: item-by-item PASS/FAIL/skip + the actual output of failures, with FAILs graded P0/P1/P2. If everything passes or only D8 is skipped → Windows can be declared upgraded from "compiles" to "usable on real hardware".
