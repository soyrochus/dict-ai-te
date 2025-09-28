pub struct LanguageOption {
    pub code: &'static str,
    pub name: &'static str,
}

pub struct VoiceOption {
    pub id: &'static str,
    pub label: &'static str,
}

pub const LANGUAGES: &[LanguageOption] = &[
    LanguageOption {
        code: "default",
        name: "Default (Auto-detect)",
    },
    LanguageOption {
        code: "en",
        name: "English",
    },
    LanguageOption {
        code: "zh",
        name: "中文 (Chinese, Mandarin)",
    },
    LanguageOption {
        code: "es",
        name: "Español (Spanish)",
    },
    LanguageOption {
        code: "de",
        name: "Deutsch (German)",
    },
    LanguageOption {
        code: "fr",
        name: "Français (French)",
    },
    LanguageOption {
        code: "ja",
        name: "日本語 (Japanese)",
    },
    LanguageOption {
        code: "pt",
        name: "Português (Portuguese)",
    },
    LanguageOption {
        code: "ru",
        name: "Русский (Russian)",
    },
    LanguageOption {
        code: "ar",
        name: "العربية (Arabic)",
    },
    LanguageOption {
        code: "it",
        name: "Italiano (Italian)",
    },
    LanguageOption {
        code: "ko",
        name: "한국어 (Korean)",
    },
    LanguageOption {
        code: "hi",
        name: "हिन्दी (Hindi)",
    },
    LanguageOption {
        code: "nl",
        name: "Nederlands (Dutch)",
    },
    LanguageOption {
        code: "tr",
        name: "Türkçe (Turkish)",
    },
    LanguageOption {
        code: "pl",
        name: "Polski (Polish)",
    },
    LanguageOption {
        code: "id",
        name: "Bahasa Indonesia (Indonesian)",
    },
    LanguageOption {
        code: "th",
        name: "ภาษาไทย (Thai)",
    },
    LanguageOption {
        code: "sv",
        name: "Svenska (Swedish)",
    },
    LanguageOption {
        code: "he",
        name: "עברית (Hebrew)",
    },
    LanguageOption {
        code: "cs",
        name: "Čeština (Czech)",
    },
];

pub const FEMALE_VOICES: &[VoiceOption] = &[
    VoiceOption {
        id: "nova",
        label: "Nova",
    },
    VoiceOption {
        id: "alloy",
        label: "Alloy",
    },
    VoiceOption {
        id: "verse",
        label: "Verse",
    },
    VoiceOption {
        id: "sol",
        label: "Sol",
    },
];

pub const MALE_VOICES: &[VoiceOption] = &[
    VoiceOption {
        id: "onyx",
        label: "Onyx",
    },
    VoiceOption {
        id: "sage",
        label: "Sage",
    },
    VoiceOption {
        id: "echo",
        label: "Echo",
    },
    VoiceOption {
        id: "ember",
        label: "Ember",
    },
];

pub const VOICE_SAMPLE_TEXT: &str = "This is a short sample to preview the selected voice.";
