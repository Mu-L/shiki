use tui_textarea::TextArea;

/// Thin wrapper over `tui-textarea` for the inline editor (key `e`).
pub struct InlineEditor<'a> {
    pub textarea: TextArea<'a>,
}

impl<'a> InlineEditor<'a> {
    pub fn from_contents(contents: &str) -> Self {
        let lines: Vec<String> = if contents.is_empty() {
            vec![String::new()]
        } else {
            contents.lines().map(str::to_string).collect()
        };
        Self {
            textarea: TextArea::new(lines),
        }
    }

    pub fn contents(&self) -> String {
        self.textarea.lines().join("\n")
    }
}
