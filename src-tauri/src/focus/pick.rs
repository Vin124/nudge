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

#[cfg(test)]
mod tests {
    use super::*;

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
