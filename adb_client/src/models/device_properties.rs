use std::collections::HashMap;

/// Commonly-used device properties, extracted from `getprop` output into typed fields.
///
/// Built from the full property map returned by [`crate::ADBDeviceExt::get_properties`].
/// Every field is optional since the underlying property may be absent on a given device.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceProperties {
    /// `ro.product.model`
    pub model: Option<String>,
    /// `ro.product.brand`
    pub brand: Option<String>,
    /// `ro.product.manufacturer`
    pub manufacturer: Option<String>,
    /// `ro.product.device`
    pub device: Option<String>,
    /// `ro.product.name`
    pub name: Option<String>,
    /// `ro.build.version.release` (e.g. `14`)
    pub android_release: Option<String>,
    /// `ro.build.version.sdk` parsed as an integer (e.g. `34`)
    pub sdk_level: Option<u32>,
    /// `ro.product.cpu.abi` (the primary ABI, e.g. `arm64-v8a`)
    pub abi: Option<String>,
    /// `ro.product.cpu.abilist` split into individual ABIs
    pub abi_list: Vec<String>,
}

impl DeviceProperties {
    /// Build typed properties from a parsed `getprop` map.
    pub(crate) fn from_property_map(props: &HashMap<String, String>) -> Self {
        let get = |key: &str| props.get(key).cloned();
        Self {
            model: get("ro.product.model"),
            brand: get("ro.product.brand"),
            manufacturer: get("ro.product.manufacturer"),
            device: get("ro.product.device"),
            name: get("ro.product.name"),
            android_release: get("ro.build.version.release"),
            sdk_level: props
                .get("ro.build.version.sdk")
                .and_then(|v| v.trim().parse().ok()),
            abi: get("ro.product.cpu.abi"),
            abi_list: props
                .get("ro.product.cpu.abilist")
                .map(|v| {
                    v.split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

/// Physical display geometry, as reported by `wm size` / `wm density`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayInfo {
    /// Display width in pixels
    pub width: u32,
    /// Display height in pixels
    pub height: u32,
    /// Display density in DPI, if reported
    pub density: Option<u32>,
}

/// Parse the output of `getprop` (lines of the form `[key]: [value]`) into a map.
pub(crate) fn parse_getprop(output: &str) -> HashMap<String, String> {
    let mut properties = HashMap::new();
    for line in output.lines() {
        // Expected format: `[key]: [value]`
        let Some(rest) = line.trim().strip_prefix('[') else {
            continue;
        };
        let Some((key, value)) = rest.split_once("]: [") else {
            continue;
        };
        let Some(value) = value.strip_suffix(']') else {
            continue;
        };
        properties.insert(key.to_string(), value.to_string());
    }
    properties
}

/// Parse a `WxH` dimension string (e.g. `1080x2340`).
fn parse_dimensions(value: &str) -> Option<(u32, u32)> {
    let (width, height) = value.trim().split_once('x')?;
    Some((width.trim().parse().ok()?, height.trim().parse().ok()?))
}

/// Parse the output of `wm size`, preferring an `Override size` over the `Physical size`
/// since the override reflects the currently-effective resolution.
pub(crate) fn parse_wm_size(output: &str) -> Option<(u32, u32)> {
    let mut physical = None;
    let mut override_size = None;
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Physical size:") {
            physical = parse_dimensions(rest);
        } else if let Some(rest) = line.strip_prefix("Override size:") {
            override_size = parse_dimensions(rest);
        }
    }
    override_size.or(physical)
}

/// Parse the output of `wm density`, preferring an `Override density` over the `Physical density`.
pub(crate) fn parse_wm_density(output: &str) -> Option<u32> {
    let mut physical = None;
    let mut override_density = None;
    for line in output.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Physical density:") {
            physical = rest.trim().parse().ok();
        } else if let Some(rest) = line.strip_prefix("Override density:") {
            override_density = rest.trim().parse().ok();
        }
    }
    override_density.or(physical)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_GETPROP: &str = "\
[ro.product.model]: [Pixel 5]
[ro.product.brand]: [google]
[ro.product.manufacturer]: [Google]
[ro.product.device]: [redfin]
[ro.product.name]: [redfin]
[ro.build.version.release]: [14]
[ro.build.version.sdk]: [34]
[ro.product.cpu.abi]: [arm64-v8a]
[ro.product.cpu.abilist]: [arm64-v8a,armeabi-v7a,armeabi]
[persist.sys.timezone]: [Europe/London]
";

    #[test]
    fn parses_getprop_lines() {
        let map = parse_getprop(SAMPLE_GETPROP);
        assert_eq!(map.get("ro.product.model").unwrap(), "Pixel 5");
        assert_eq!(map.get("ro.build.version.sdk").unwrap(), "34");
        assert_eq!(map.get("persist.sys.timezone").unwrap(), "Europe/London");
        // Malformed / empty lines must be ignored.
        assert!(!map.contains_key(""));
    }

    #[test]
    fn builds_typed_properties() {
        let props = DeviceProperties::from_property_map(&parse_getprop(SAMPLE_GETPROP));
        assert_eq!(props.model.as_deref(), Some("Pixel 5"));
        assert_eq!(props.manufacturer.as_deref(), Some("Google"));
        assert_eq!(props.sdk_level, Some(34));
        assert_eq!(props.abi.as_deref(), Some("arm64-v8a"));
        assert_eq!(props.abi_list, vec!["arm64-v8a", "armeabi-v7a", "armeabi"]);
    }

    #[test]
    fn parses_wm_size_prefers_override() {
        assert_eq!(
            parse_wm_size("Physical size: 1080x2340"),
            Some((1080, 2340))
        );
        assert_eq!(
            parse_wm_size("Physical size: 1080x2340\nOverride size: 720x1560"),
            Some((720, 1560))
        );
        assert_eq!(parse_wm_size("garbage"), None);
    }

    #[test]
    fn parses_wm_density_prefers_override() {
        assert_eq!(parse_wm_density("Physical density: 440"), Some(440));
        assert_eq!(
            parse_wm_density("Physical density: 440\nOverride density: 420"),
            Some(420)
        );
        assert_eq!(parse_wm_density("nope"), None);
    }
}
