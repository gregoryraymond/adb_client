/// A single socket forwarding rule, as listed by `adb forward --list` / `adb reverse --list`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForwardRule {
    /// Device serial (or transport identifier) the rule belongs to, when the server reports it.
    pub serial: Option<String>,
    /// Local endpoint spec, e.g. `tcp:1234`.
    pub local: String,
    /// Remote (device-side) endpoint spec, e.g. `localabstract:minicap`.
    pub remote: String,
}

/// Parse the output of `host:list-forward` (lines of `<serial> <local> <remote>`).
pub(crate) fn parse_forward_list(output: &str) -> Vec<ForwardRule> {
    parse_rule_list(output, false)
}

/// Parse the output of `reverse:list-forward`.
///
/// Reverse rules are listed with the remote endpoint first, mirroring the wire order of
/// `reverse:forward:<remote>;<local>`.
pub(crate) fn parse_reverse_list(output: &str) -> Vec<ForwardRule> {
    parse_rule_list(output, true)
}

fn parse_rule_list(output: &str, reverse: bool) -> Vec<ForwardRule> {
    output
        .lines()
        .filter_map(|line| {
            // Lines are `<serial> <a> <b>`, or `<a> <b>` when no serial is reported.
            let (serial, first, second) = match line.split_whitespace().collect::<Vec<_>>()[..] {
                [serial, a, b] => (Some(serial.to_string()), a, b),
                [a, b] => (None, a, b),
                _ => return None,
            };
            // Forward lists order the columns `local remote`; reverse lists `remote local`.
            let (local, remote) = if reverse {
                (second, first)
            } else {
                (first, second)
            };
            Some(ForwardRule {
                serial,
                local: local.to_string(),
                remote: remote.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_forward_list() {
        let output = "\
emulator-5554 tcp:1234 tcp:5678
emulator-5554 tcp:9000 localabstract:minicap
";
        let rules = parse_forward_list(output);
        assert_eq!(rules.len(), 2);
        assert_eq!(
            rules[0],
            ForwardRule {
                serial: Some("emulator-5554".into()),
                local: "tcp:1234".into(),
                remote: "tcp:5678".into(),
            }
        );
        assert_eq!(rules[1].remote, "localabstract:minicap");
        assert_eq!(rules[1].local, "tcp:9000");
    }

    #[test]
    fn parses_reverse_list_remote_first() {
        let output = "emulator-5554 localabstract:minitouch tcp:1717\n";
        let rules = parse_reverse_list(output);
        assert_eq!(
            rules[0],
            ForwardRule {
                serial: Some("emulator-5554".into()),
                local: "tcp:1717".into(),
                remote: "localabstract:minitouch".into(),
            }
        );
    }

    #[test]
    fn parses_lines_without_serial() {
        let rules = parse_forward_list("tcp:1234 tcp:5678\n");
        assert_eq!(rules[0].serial, None);
        assert_eq!(rules[0].local, "tcp:1234");
        assert_eq!(rules[0].remote, "tcp:5678");
    }

    #[test]
    fn ignores_blank_and_malformed_lines() {
        let rules = parse_forward_list("\n\nonlyonetoken\n");
        assert!(rules.is_empty());
    }
}
