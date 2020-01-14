use std::io::Error;

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    if let Err(e) = uninstall_service() {
        show_msgbox(format!("{}. Run as admin.", e).as_str());
    } else {
        show_msgbox("Uninstalled successfully.");
    }

    Ok(())
}

#[cfg(windows)]
fn uninstall_service() -> Result<(), String> {
    use std::{thread, time::Duration};
    use windows_service::{
        service::{ServiceAccess, ServiceState},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };

    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(
        None::<&str>, manager_access
    ).map_err(|e| {
        format!("Error acquiring service manager ({})", e)
    })?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
    let service = service_manager.open_service(
        "rs_google_photos_sync", service_access
    ).map_err(|e| {
        format!("Error opening service ({})", e)
    })?;

    let service_status = service.query_status().map_err(|e| {
        format!("Error querying service ({})", e)
    })?;
    if service_status.current_state != ServiceState::Stopped {
        service.stop().map_err(|e| {
            format!("Error stopping service ({})", e)
        })?;
        // Wait for service to stop
        thread::sleep(Duration::from_secs(1));
    }

    println!("Removing service");
    service.delete().map_err(|e| {
        format!("Error deleting service {}", e)
    })?;

    Ok(())
}

#[cfg(windows)]
fn show_msgbox(msg: &str) {
    use std::ffi::OsStr;
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use winapi::um::winuser::{MB_OK, MessageBoxW};

    let wide: Vec<u16> = OsStr::new(msg).encode_wide().chain(once(0)).collect();
    let ret = unsafe {
        MessageBoxW(null_mut(), wide.as_ptr(), wide.as_ptr(), MB_OK)
    };
    let x = if ret == 0 {
        Err(Error::last_os_error())
    } else {
        Ok(ret)
    };

    println!("{:#?}", x);
}

#[cfg(not(windows))]
fn main() {
    panic!("This program is only intended to run on Windows.");
}