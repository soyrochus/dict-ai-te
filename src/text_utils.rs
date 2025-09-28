use once_cell::sync::Lazy;
use regex::Regex;

static PARA_SPLIT: Lazy<Regex> = Lazy::new(|| Regex::new(r"\n\s*\n").unwrap());
static SPACE_COLLAPSE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());

pub fn format_structured_text(text: &str) -> String {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    let mut paragraphs = Vec::new();
    for block in PARA_SPLIT.split(trimmed) {
        let block = block.trim();
        if block.is_empty() {
            continue;
        }
        let mut lines = Vec::new();
        for line in block.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let collapsed = SPACE_COLLAPSE.replace_all(line, " ");
            lines.push(collapsed);
        }
        if lines.is_empty() {
            continue;
        }
        paragraphs.push(lines.join(" "));
    }

    paragraphs.join("\n\n")
}
