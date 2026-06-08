/// Describes which set of packages [`crate::ADBDeviceExt::list_packages`] should return,
/// how much detail to include for each entry, and which user to query.
#[derive(Debug)]
pub enum PackageListType {
    /// All packages (excluding APEX containers), equivalent to `pm list packages -a`
    AllNonApex(PackageDetails, UserFilter),
    /// Only APEX (Android Pony Express) packages, equivalent to `pm list packages --apex-only`
    Apex(PackageDetails, UserFilter),
    /// Only disabled packages, equivalent to `pm list packages -d`
    Disabled(PackageDetails, UserFilter),
    /// Only enabled packages, equivalent to `pm list packages -e`
    Enabled(PackageDetails, UserFilter),
    /// Only system packages, equivalent to `pm list packages -s`
    System(PackageDetails, UserFilter),
    /// Only uninstalled packages whose data is kept, equivalent to `pm list packages -u`
    Uninstalled(PackageDetails, UserFilter),
}

/// Describes how much detail to include for each listed package.
#[derive(Debug)]
pub enum PackageDetails {
    /// Just the package identifier, as output by a plain `pm list packages`
    Normal,
    /// Also include the version code, equivalent to `--show-versioncode`
    ShowVersionCode,
    /// Also include the installer, equivalent to `-i`
    ShowInstaller,
    /// Also include the associated APK path, equivalent to `-f`
    ShowAssociatedApks,
}

/// Describes how the listing should be filtered by user, if at all.
#[derive(Debug)]
pub enum UserFilter {
    /// Do not pass any user information to the command
    NoUserSpecified,
    /// Resolve the device's current user and filter on it
    CurrentUser,
    /// Filter on an explicitly provided user id
    SpecificUser(u32),
}

impl PackageListType {
    /// Decompose this list type into its `pm list packages` filter flag, the requested
    /// level of detail and the user filter to apply.
    pub(crate) fn components(&self) -> (&'static str, &PackageDetails, &UserFilter) {
        match self {
            PackageListType::AllNonApex(details, user) => ("-a", details, user),
            PackageListType::Apex(details, user) => ("--apex-only", details, user),
            PackageListType::Disabled(details, user) => ("-d", details, user),
            PackageListType::Enabled(details, user) => ("-e", details, user),
            PackageListType::System(details, user) => ("-s", details, user),
            PackageListType::Uninstalled(details, user) => ("-u", details, user),
        }
    }
}

impl PackageDetails {
    /// Returns the `pm list packages` flag enabling this level of detail, if any.
    pub(crate) fn flag(&self) -> Option<&'static str> {
        match self {
            PackageDetails::Normal => None,
            PackageDetails::ShowVersionCode => Some("--show-versioncode"),
            PackageDetails::ShowInstaller => Some("-i"),
            PackageDetails::ShowAssociatedApks => Some("-f"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_filters_to_flags() {
        let cases = [
            (
                PackageListType::AllNonApex(PackageDetails::Normal, UserFilter::NoUserSpecified),
                "-a",
            ),
            (
                PackageListType::Apex(PackageDetails::Normal, UserFilter::NoUserSpecified),
                "--apex-only",
            ),
            (
                PackageListType::Disabled(PackageDetails::Normal, UserFilter::NoUserSpecified),
                "-d",
            ),
            (
                PackageListType::Enabled(PackageDetails::Normal, UserFilter::NoUserSpecified),
                "-e",
            ),
            (
                PackageListType::System(PackageDetails::Normal, UserFilter::NoUserSpecified),
                "-s",
            ),
            (
                PackageListType::Uninstalled(PackageDetails::Normal, UserFilter::NoUserSpecified),
                "-u",
            ),
        ];

        for (list_type, expected_flag) in cases {
            assert_eq!(list_type.components().0, expected_flag);
        }
    }

    #[test]
    fn maps_detail_to_flags() {
        assert_eq!(PackageDetails::Normal.flag(), None);
        assert_eq!(
            PackageDetails::ShowVersionCode.flag(),
            Some("--show-versioncode")
        );
        assert_eq!(PackageDetails::ShowInstaller.flag(), Some("-i"));
        assert_eq!(PackageDetails::ShowAssociatedApks.flag(), Some("-f"));
    }
}
