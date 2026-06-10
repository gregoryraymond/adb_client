use crate::{
    Result,
    models::{ADBCommand, ADBLocalCommand},
    server_device::ADBServerDevice,
};

impl ADBServerDevice {
    /// List the PIDs of debuggable (JDWP) processes currently on the device.
    ///
    /// This reads a single snapshot from the device's `track-jdwp` service. The service would
    /// otherwise keep streaming updates as debuggable processes come and go.
    pub fn list_jdwp(&mut self) -> Result<Vec<u32>> {
        self.set_serial_transport()?;

        let response = self
            .transport
            .proxy_connection(&ADBCommand::Local(ADBLocalCommand::TrackJdwp), true)?;

        Ok(parse_jdwp_pids(&String::from_utf8(response)?))
    }
}

/// Parse the newline-separated PID list emitted by `track-jdwp`.
fn parse_jdwp_pids(output: &str) -> Vec<u32> {
    output
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pid_list() {
        assert_eq!(
            parse_jdwp_pids("1234\n5678\n9012\n"),
            vec![1234, 5678, 9012]
        );
    }

    #[test]
    fn ignores_blank_and_garbage_lines() {
        assert_eq!(
            parse_jdwp_pids("\n1234\n\nnot-a-pid\n5678\n"),
            vec![1234, 5678]
        );
    }

    #[test]
    fn empty_output_is_empty() {
        assert!(parse_jdwp_pids("").is_empty());
    }
}
