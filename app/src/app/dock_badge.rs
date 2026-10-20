//! The Dock badge's platform boundary: the macOS Dock, or a Linux dock
//! that draws badges.

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub(super) use macos::Badge;

#[cfg(not(target_os = "macos"))]
mod linux;
#[cfg(not(target_os = "macos"))]
pub(super) use linux::Badge;
