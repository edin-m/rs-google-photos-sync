use crate::error::CustomResult;

#[cfg(windows)]
pub fn install_service() {
    println!("installing service on windows");
    xxx();
}

#[cfg(windows)]
pub fn xxx() -> CustomResult<()> {
    println!("installing xxx on windows");
    Ok(())
}

#[cfg(windows)]
pub fn yyy() -> windows_service::Result<()> {
    println!("installing yyy on windows");
    Ok(())
}

#[cfg(windows)]
pub fn uninstall_service() {
    println!("uninstalling service on windows");
}

#[cfg(not(windows))]
pub fn install_service() {
    panic!("Service not supported!");
}

#[cfg(not(windows))]
pub fn uninstall_service() {
    panic!("Service not supported!");
}