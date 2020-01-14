use std::io::Error;

#[cfg(windows)]
fn main() -> windows_service::Result<()> {
    install_service()?;
    Ok(())
}

#[cfg(windows)]
fn install_service() -> windows_service::Result<()> {
    use std::ffi::OsString;
    use windows_service::{
        service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };

    println!("Accessing service manager");
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(
        None::<&str>, manager_access
    )?;

    let service_binary_path = ::std::env::current_exe()
        .unwrap()
        .with_file_name("rs_google_photos_sync.exe");

    let service_info = ServiceInfo {
        name: OsString::from("rs_google_photos_sync"),
        display_name: OsString::from("Sync-down Google Photos service"),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::OnDemand,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None, // run as System
        account_password: None,
    };

    println!("creating service");
    let result = service_manager.create_service(service_info, ServiceAccess::empty());

    if result.is_err() {
        show_msgbox("Error installing service");
        println!("{}", result.err().unwrap());
    } else {
        show_msgbox("Service installed successfully");
    }

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
    panic!("not supported");
}