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

/// Escape a string so it can be passed as the single argument to `input text`.
///
/// `input text` uses `%s` for spaces, and the text travels through a device-side shell,
/// so shell-special characters are backslash-escaped. A literal `%` cannot be represented
/// (it collides with the `%s` space convention) and is dropped by the device.
pub(crate) fn escape_input_text(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            ' ' => escaped.push_str("%s"),
            '(' | ')' | '<' | '>' | '|' | ';' | '&' | '*' | '\\' | '~' | '"' | '\'' | '`' | '$'
            | '#' | '!' | '?' | '{' | '}' | '[' | ']' => {
                escaped.push('\\');
                escaped.push(c);
            }
            _ => escaped.push(c),
        }
    }
    escaped
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
    fn escapes_spaces_as_percent_s() {
        assert_eq!(escape_input_text("hello world"), "hello%sworld");
    }

    #[test]
    fn escapes_shell_specials() {
        assert_eq!(escape_input_text("a&b"), "a\\&b");
        assert_eq!(escape_input_text("$(x)"), "\\$\\(x\\)");
        assert_eq!(escape_input_text("plain123"), "plain123");
    }
}
