use std::collections::BTreeMap;

#[derive(Default)]
pub struct TranscriptAssembler {
    order: Vec<String>,
    segments: BTreeMap<String, Segment>,
    anonymous: Vec<String>,
}

#[derive(Default)]
struct Segment {
    text: String,
    final_text: bool,
}

impl TranscriptAssembler {
    pub fn add_delta(&mut self, item_id: Option<&str>, text: &str) {
        if text.is_empty() {
            return;
        }
        let Some(item_id) = item_id else {
            self.anonymous.push(text.to_string());
            return;
        };
        if !self.segments.contains_key(item_id) {
            self.order.push(item_id.to_string());
        }
        let segment = self.segments.entry(item_id.to_string()).or_default();
        if !segment.final_text {
            segment.text.push_str(text);
        }
    }

    pub fn complete(&mut self, item_id: Option<&str>, text: &str) {
        if text.is_empty() {
            return;
        }
        let Some(item_id) = item_id else {
            self.anonymous.push(text.to_string());
            return;
        };
        if !self.segments.contains_key(item_id) {
            self.order.push(item_id.to_string());
        }
        let segment = self.segments.entry(item_id.to_string()).or_default();
        segment.text = text.to_string();
        segment.final_text = true;
    }

    pub fn text(&self) -> String {
        let mut parts = Vec::new();
        for item_id in &self.order {
            if let Some(segment) = self.segments.get(item_id) {
                if !segment.text.trim().is_empty() {
                    parts.push(segment.text.trim().to_string());
                }
            }
        }
        parts.extend(
            self.anonymous
                .iter()
                .filter(|item| !item.trim().is_empty())
                .cloned(),
        );
        parts.join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_partial_without_duplicate() {
        let mut assembler = TranscriptAssembler::default();
        assembler.add_delta(Some("b"), "second");
        assembler.add_delta(Some("a"), "fir");
        assembler.complete(Some("a"), "first");
        assert_eq!(assembler.text(), "second first");
    }

    #[test]
    fn handles_anonymous_delta_and_out_of_order_completion() {
        let mut assembler = TranscriptAssembler::default();
        assembler.add_delta(Some("item-2"), "world");
        assembler.add_delta(None, "loose");
        assembler.complete(Some("item-1"), "hello");
        assembler.complete(Some("item-2"), "world");
        assert_eq!(assembler.text(), "world hello loose");
    }
}
