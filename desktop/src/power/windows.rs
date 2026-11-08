use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};

/// Windows battery saver (energy saver): `SystemStatusFlag` is 1 while it is on.
pub fn saving() -> bool {
    let mut status = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut status) != 0 && status.SystemStatusFlag == 1 }
}
