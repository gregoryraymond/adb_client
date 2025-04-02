use std::fmt::Display;

#[derive(Debug)]
/// PackageListType describes which types of packages should be displayed in the list, along with how detailed a view, and user filtering
pub enum PackageListType {
    /// Present a list of all non-APEX (Android Pony EXpress) packages equivalent to '-a'
    AllNonApex(PackageDetails, UserFilter),
    /// Present a list of only APEX packages, equivalent to '--apex-only'
    Apex(PackageDetails, UserFilter),
    /// Present a list of only disabled packages, equivalent to '-d'
    Disabled(PackageDetails, UserFilter),
    /// Present a list of only enabled packages, equivalent to '-e'
    Enabled(PackageDetails, UserFilter),
    /// Present a list of only system packages, equivalent to '-s'
    System(PackageDetails, UserFilter),
    /// Present a list of only uninstalled packages, equivalent to '-u'
    Uninstalled(PackageDetails, UserFilter)
}

#[derive(Debug)]
/// PAckage details describes how much detail to extract when doing the list command
pub enum PackageDetails {
    /// Present the package details as a normal output as if outputting via adb shell
    Normal,
    /// Present the package details with associated version codes
    ShowVersionCode,
    /// Present the package details with the associated installer
    ShowInstaller,
    /// Present the package details with the associated apks
    ShowAssociatedApks
}

#[derive(Debug)]
/// User Filter describes how the command should filter on the user if at all
pub enum UserFilter {
    /// If no user is specified, user information is not supplied to the command
    NoUserSpecified,
    /// If the current user is requested we attempt to get the current user and use it as the filter on the command
    CurrentUser,
    /// If the user is known it can be explicitly stated
    SpecificUser(u32)
}

impl Display for PackageListType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            _ => write!(f, ""),
        }
    }
}
