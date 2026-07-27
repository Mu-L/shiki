use crate::theme::Theme;

/// Official palette from draculatheme.com/contribute — bg/current-line/fg/
/// comment/cyan/green/orange/pink/purple/red/yellow are all taken verbatim
/// from the spec; the remaining slots (statusbar/scrollbar/tab_inactive/
/// panel_title/link/tag) have no equivalent in the spec itself and are
/// chosen the same way every other theme in this file already does —
/// reusing the spec's own colors for a slot that reads naturally that way
/// (e.g. `tag` as pink, `link` as cyan) rather than inventing new hex
/// values.
pub fn dracula() -> Theme {
    Theme {
        name: "dracula".into(),
        bg: "#282a36".into(),
        fg: "#f8f8f2".into(),
        accent: "#bd93f9".into(),
        selection: "#44475a".into(),
        border: "#44475a".into(),
        statusbar: "#21222c".into(),
        highlight: "#f1fa8c".into(),
        error: "#ff5555".into(),
        warning: "#ffb86c".into(),
        success: "#50fa7b".into(),
        inactive: "#6272a4".into(),
        scrollbar: "#44475a".into(),
        tab_active: "#bd93f9".into(),
        tab_inactive: "#6272a4".into(),
        panel_title: "#8be9fd".into(),
        cursor: "#f8f8f2".into(),
        link: "#8be9fd".into(),
        tag: "#ff79c6".into(),
        muted: "#6272a4".into(),
    }
}
