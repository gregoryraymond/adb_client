/// A subset of Android key codes, for use with [`crate::ADBDeviceExt::input_keyevent`].
///
/// Values match the constants in `android.view.KeyEvent`. The list is intentionally
/// non-exhaustive; pass an arbitrary code as an integer when a constant is missing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[repr(i32)]
pub enum KeyCode {
    /// `KEYCODE_HOME`
    Home = 3,
    /// `KEYCODE_BACK`
    Back = 4,
    /// `KEYCODE_DPAD_UP`
    DpadUp = 19,
    /// `KEYCODE_DPAD_DOWN`
    DpadDown = 20,
    /// `KEYCODE_DPAD_LEFT`
    DpadLeft = 21,
    /// `KEYCODE_DPAD_RIGHT`
    DpadRight = 22,
    /// `KEYCODE_DPAD_CENTER`
    DpadCenter = 23,
    /// `KEYCODE_VOLUME_UP`
    VolumeUp = 24,
    /// `KEYCODE_VOLUME_DOWN`
    VolumeDown = 25,
    /// `KEYCODE_POWER`
    Power = 26,
    /// `KEYCODE_ENTER`
    Enter = 66,
    /// `KEYCODE_DEL` (backspace)
    Del = 67,
    /// `KEYCODE_MENU`
    Menu = 82,
    /// `KEYCODE_APP_SWITCH` (recents)
    AppSwitch = 187,
}

impl KeyCode {
    /// Return the integer Android key code.
    #[must_use]
    pub fn code(self) -> i32 {
        self as i32
    }
}

impl From<KeyCode> for i32 {
    fn from(value: KeyCode) -> Self {
        value.code()
    }
}

/// Quote `text` so it is passed verbatim as the single argument to `input text`.
///
/// The text travels through a device-side shell, so it is wrapped in single quotes — which
/// make every byte literal (spaces, newlines, shell metacharacters) — with any embedded
/// single quote escaped as `'\''`. This neutralizes command-splitting / injection that a
/// metacharacter denylist would miss (most importantly a newline, which would otherwise act
/// as a command separator).
pub(crate) fn escape_input_text(text: &str) -> String {
    let mut quoted = String::with_capacity(text.len() + 2);
    quoted.push('\'');
    for c in text.chars() {
        if c == '\'' {
            // Close the quote, emit an escaped quote, reopen the quote.
            quoted.push_str("'\\''");
        } else {
            quoted.push(c);
        }
    }
    quoted.push('\'');
    quoted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keycode_converts_to_int() {
        assert_eq!(KeyCode::Home.code(), 3);
        assert_eq!(i32::from(KeyCode::AppSwitch), 187);
    }

    #[test]
    fn quotes_plain_text_and_spaces() {
        assert_eq!(escape_input_text("hello world"), "'hello world'");
        assert_eq!(escape_input_text("plain123"), "'plain123'");
    }

    #[test]
    fn neutralizes_metacharacters_and_newlines() {
        // Metacharacters and newlines stay inside the quotes — no shell token escapes.
        assert_eq!(escape_input_text("a&b;c|d"), "'a&b;c|d'");
        assert_eq!(escape_input_text("a\nrm -rf /"), "'a\nrm -rf /'");
        assert_eq!(escape_input_text("$(whoami)"), "'$(whoami)'");
    }

    #[test]
    fn escapes_embedded_single_quote() {
        assert_eq!(escape_input_text("it's"), "'it'\\''s'");
    }
}
