use ledger_device_sdk::include_gif;
use ledger_device_sdk::nbgl::*;

#[cfg(any(target_os = "nanosplus", target_os = "nanox"))]
pub const APP_ICON_HOME: NbglGlyph =
    NbglGlyph::from_include(include_gif!("home_nano_nbgl.png", NBGL));
#[cfg(any(target_os = "nanosplus", target_os = "nanox"))]
pub const APP_ICON: NbglGlyph = NbglGlyph::from_include(include_gif!("sui-small.gif", NBGL));
#[cfg(target_os = "apex_p")]
pub const APP_ICON: NbglGlyph = NbglGlyph::from_include(include_gif!("sui_48x48.png", NBGL));
#[cfg(any(target_os = "stax", target_os = "flex"))]
pub const APP_ICON: NbglGlyph = NbglGlyph::from_include(include_gif!("sui_64x64.gif", NBGL));
#[cfg(any(target_os = "stax", target_os = "flex", target_os = "apex_p"))]
pub const APP_ICON_HOME: NbglGlyph = APP_ICON;

pub mod nbgl;
//pub mod common;
