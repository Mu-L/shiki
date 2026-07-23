//! Opens a URL in the user's default browser — used by the footer's Buy Me
//! a Coffee link. Each OS has its own "hand this URL to whatever's
//! registered as the default browser" command; there's no cross-platform
//! standard binary for it the way `xdg-mime`/`open -W -t` cover editors.

/// Spawns the OS's default-browser opener for `url`. Fire-and-forget: the
/// caller doesn't wait on it, matching how external-editor spawns elsewhere
/// in this codebase are best-effort (`let _ = ...`).
pub fn open_url(url: &str) -> std::io::Result<std::process::Child> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(url).spawn()
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(url).spawn()
    }
    #[cfg(target_os = "windows")]
    {
        // `start`'s first argument after the shell built-in is treated as a
        // window title if quoted, so an empty title arg is required —
        // without it, a URL containing `&` gets misparsed as the title.
        std::process::Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
    }
}
