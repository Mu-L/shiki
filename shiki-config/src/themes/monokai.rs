use crate::theme::Theme;

/// The classic Monokai palette (Wimer Hazenberg's original Sublime
/// Text/TextMate scheme) — bg/fg/comment/pink/orange/yellow/green/cyan/
/// purple match the values that have stayed identical across every
/// faithful port since. Not "Monokai Pro" (a separate, later, paid
/// product with a different, cooler-toned palette) — this is the
/// original, still the far more widely recognized of the two.
pub fn monokai() -> Theme {
    Theme {
        name: "monokai".into(),
        bg: "#272822".into(),
        fg: "#f8f8f2".into(),
        accent: "#f92672".into(),
        selection: "#49483e".into(),
        border: "#3e3d32".into(),
        statusbar: "#1e1f1c".into(),
        highlight: "#e6db74".into(),
        error: "#f92672".into(),
        warning: "#fd971f".into(),
        success: "#a6e22e".into(),
        inactive: "#75715e".into(),
        scrollbar: "#49483e".into(),
        tab_active: "#f92672".into(),
        tab_inactive: "#75715e".into(),
        panel_title: "#66d9ef".into(),
        cursor: "#f8f8f2".into(),
        link: "#66d9ef".into(),
        tag: "#ae81ff".into(),
        muted: "#75715e".into(),
    }
}
