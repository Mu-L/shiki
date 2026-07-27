use crate::theme::Theme;

/// The canonical "Atom One Dark" palette — bg/fg/red/green/yellow/orange/
/// blue/purple/cyan/comment-grey match the original atom-one-dark-syntax
/// values (the same ones every faithful one-dark port, vim or otherwise,
/// still uses unchanged years later); `cursor` uses One Dark's own
/// distinctive caret blue (`#528bff`) rather than reusing `fg`, since it's
/// as recognizable a part of the theme's identity as the accent color.
pub fn one_dark() -> Theme {
    Theme {
        name: "one-dark".into(),
        bg: "#282c34".into(),
        fg: "#abb2bf".into(),
        accent: "#61afef".into(),
        selection: "#3e4451".into(),
        border: "#3e4451".into(),
        statusbar: "#21252b".into(),
        highlight: "#e5c07b".into(),
        error: "#e06c75".into(),
        warning: "#d19a66".into(),
        success: "#98c379".into(),
        inactive: "#5c6370".into(),
        scrollbar: "#3e4451".into(),
        tab_active: "#61afef".into(),
        tab_inactive: "#5c6370".into(),
        panel_title: "#c678dd".into(),
        cursor: "#528bff".into(),
        link: "#61afef".into(),
        tag: "#c678dd".into(),
        muted: "#5c6370".into(),
    }
}
