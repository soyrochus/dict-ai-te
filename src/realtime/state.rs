#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiveState {
    Disconnected,
    Transcribing,
    Translating,
    Error,
}

impl LiveState {
    pub fn connected(translate: bool) -> Self {
        if translate {
            LiveState::Translating
        } else {
            LiveState::Transcribing
        }
    }

    pub fn stop(self) -> Self {
        match self {
            LiveState::Transcribing | LiveState::Translating => LiveState::Disconnected,
            other => other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moves_through_live_states() {
        assert_eq!(LiveState::connected(false), LiveState::Transcribing);
        assert_eq!(LiveState::connected(true), LiveState::Translating);
        assert_eq!(LiveState::Translating.stop(), LiveState::Disconnected);
    }
}
