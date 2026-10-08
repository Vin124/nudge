//! Pure choice of which ancestor process hosts the session's terminal window.

use nudge_proto::ProcInfo;

/// Shells, the Claude process itself, and console plumbing: never the window owner.
const SKIP: &[&str] = &[
    "claude", "node", "bash", "sh", "zsh", "fish", "cmd", "powershell", "pwsh", "conhost", "openconsole", "wsl",
    "wslhost", "login", "tmux", "screen", "sudo",
    // Beyond the brief: OS session parents. They own unrelated windows (File
    // Explorer, the desktop), so focusing them would be wrong; fall back instead.
    "explorer", "sihost", "svchost", "services", "wininit", "winlogon", "userinit", "launchd", "init", "systemd",
];

/// Terminal / editor hosts we know own the window. Preferred wherever they sit in the chain.
const KNOWN_HOSTS: &[&str] = &[
    "windowsterminal", "code", "code - insiders", "cursor", "idea64", "terminal", "iterm2", "warp", "wezterm",
    "wezterm-gui", "alacritty", "kitty", "ghostty", "hyper",
];

/// Lower-cased name without a trailing `.exe`.
pub fn norm(name: &str) -> String {
    let n = name.trim().to_ascii_lowercase();
    n.strip_suffix(".exe").map(str::to_owned).unwrap_or(n)
}

pub fn is_known_host(name: &str) -> bool {
    KNOWN_HOSTS.contains(&norm(name).as_str())
}

/// Candidate pids, best first: known hosts (nearest first), then the remaining
/// non-skipped ancestors (nearest first). Empty if the chain is all shells.
pub fn candidates(ancestors: &[ProcInfo]) -> Vec<u32> {
    let usable: Vec<&ProcInfo> = ancestors.iter().filter(|p| !SKIP.contains(&norm(&p.name).as_str())).collect();
    let known = usable.iter().filter(|p| is_known_host(&p.name));
    let other = usable.iter().filter(|p| !is_known_host(&p.name));
    let mut out: Vec<u32> = Vec::new();
    for p in known.chain(other) {
        if !out.contains(&p.pid) {
            out.push(p.pid);
        }
    }
    out
}

/// Index of the window whose title names the session's folder, else 0 (z-order front).
/// One host process often owns many windows (VS Code, Windows Terminal), and editors
/// title them "file - folder - App". Folders are tried deepest first, so a session
/// in a subfolder still finds the window opened on its parent workspace.
pub fn best_window(titles: &[String], cwd: &str) -> usize {
    let folders: Vec<String> =
        cwd.split(['/', '\\']).filter(|c| !c.is_empty() && !c.ends_with(':')).map(str::to_lowercase).collect();
    let names_folder = |title: &str, folder: &str| {
        title.to_lowercase().split(" - ").map(str::trim).any(|seg| {
            // "proj [WSL: Ubuntu]", "proj (Workspace)"
            seg == folder || seg.strip_prefix(folder).is_some_and(|r| r.starts_with(" [") || r.starts_with(" ("))
        })
    };
    folders.iter().rev().find_map(|f| titles.iter().position(|t| names_folder(t, f))).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn titles(t: &[&str]) -> Vec<String> {
        t.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn picks_the_vscode_window_for_the_session_folder() {
        // Real titles: five windows, all owned by one Code.exe, wrong one in front.
        let t = titles(&[
            ".env.example - cluely - Visual Studio Code",
            "Welcome - ML4FG - Visual Studio Code",
            "brag.mp4 - claude-noti - Visual Studio Code",
            "msr-undergrad-essays.md - internship 2027 - Visual Studio Code",
        ]);
        assert_eq!(best_window(&t, "C:\\Users\\Vin\\Documents\\claude-noti"), 2);
        assert_eq!(best_window(&t, "C:\\Users\\Vin\\internship 2027\\"), 3);
        // Subfolder session: falls back to the workspace folder above it.
        assert_eq!(best_window(&t, "C:\\Users\\Vin\\Documents\\claude-noti\\src-tauri"), 2);
    }

    #[test]
    fn falls_back_to_front_window() {
        let t = titles(&["✳ Claude Code", "pwsh"]);
        assert_eq!(best_window(&t, "C:\\work\\api"), 0);
        assert_eq!(best_window(&[], "C:\\work\\api"), 0);
        assert_eq!(best_window(&t, ""), 0);
    }

    #[test]
    fn no_substring_false_positives_and_suffixes_ok() {
        let t = titles(&["a.rs - api-v2 - Cursor", "b.rs - api [WSL: Ubuntu] - Visual Studio Code"]);
        assert_eq!(best_window(&t, "/home/u/api"), 1);
        let ws = titles(&["x - other - Visual Studio Code", "x - Proj (Workspace) - Visual Studio Code"]);
        assert_eq!(best_window(&ws, "/p/proj"), 1);
    }

    fn chain(names: &[&str]) -> Vec<ProcInfo> {
        names.iter().enumerate().map(|(i, n)| ProcInfo { pid: 100 + i as u32, name: (*n).into() }).collect()
    }

    #[test]
    fn windows_terminal_chain() {
        let a = chain(&["claude.exe", "pwsh.exe", "OpenConsole.exe", "WindowsTerminal.exe", "explorer.exe"]);
        assert_eq!(candidates(&a), vec![103]);
    }

    #[test]
    fn vscode_chain() {
        let a = chain(&["claude.exe", "powershell.exe", "Code.exe", "Code.exe", "explorer.exe"]);
        assert_eq!(candidates(&a), vec![102, 103]);
    }

    #[test]
    fn npm_installed_claude_under_node() {
        let a = chain(&["cmd.exe", "node.exe", "cmd.exe", "bash.exe", "WindowsTerminal.exe"]);
        assert_eq!(candidates(&a), vec![104]);
    }

    #[test]
    fn all_shells_is_empty() {
        let a = chain(&["claude", "zsh", "login", "tmux", "sudo", "sh", "fish", "wsl.exe", "wslhost.exe", "conhost.exe", "screen", "explorer.exe", "launchd"]);
        assert!(candidates(&a).is_empty());
        assert!(candidates(&[]).is_empty());
    }

    #[test]
    fn case_and_exe_insensitive() {
        let a = chain(&["CLAUDE.EXE", "PwSh.Exe", "windowsterminal.EXE"]);
        assert_eq!(candidates(&a), vec![102]);
        assert!(is_known_host("Code - Insiders.exe"));
        assert!(is_known_host("iTerm2"));
        assert_eq!(norm(" Cursor.EXE "), "cursor");
    }

    #[test]
    fn prefers_known_host_deeper_in_chain() {
        // An unknown wrapper (e.g. a launcher) sits nearer than the real host.
        let a = chain(&["claude", "zsh", "some-launcher", "iTerm2", "launchd"]);
        assert_eq!(candidates(&a), vec![103, 102]);
    }
}
