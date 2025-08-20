#[cfg(any(target_os = "stax", target_os = "flex", target_os = "apex_p"))]
pub mod nbgl;
#[cfg(any(target_os = "stax", target_os = "flex", target_os = "apex_p"))]
pub use nbgl::*;

#[cfg(not(any(target_os = "stax", target_os = "flex", target_os = "apex_p")))]
pub mod nano;
#[cfg(not(any(target_os = "stax", target_os = "flex", target_os = "apex_p")))]
pub use nano::*;

pub mod common;
