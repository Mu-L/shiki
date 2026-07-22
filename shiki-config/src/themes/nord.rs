use crate::theme::Theme;

pub fn nord() -> Theme {
    Theme {
        name: "nord".into(),
        bg: "#2e3440".into(),
        fg: "#d8dee9".into(),
        accent: "#88c0d0".into(),
        selection: "#434c5e".into(),
        border: "#3b4252".into(),
        statusbar: "#242933".into(),
        highlight: "#ebcb8b".into(),
        error: "#bf616a".into(),
        warning: "#d08770".into(),
        success: "#a3be8c".into(),
        inactive: "#4c566a".into(),
        scrollbar: "#3b4252".into(),
        tab_active: "#b48ead".into(),
        tab_inactive: "#4c566a".into(),
        panel_title: "#81a1c1".into(),
        cursor: "#d8dee9".into(),
        link: "#5e81ac".into(),
        tag: "#b48ead".into(),
        muted: "#8fbcbb".into(),
    }
}
