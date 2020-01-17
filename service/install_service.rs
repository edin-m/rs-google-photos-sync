mod show_msg;

#[cfg(windows)]
fn main() {
    if let Err(e) = install_service() {
        println!("{}", e);
        show_msg::show_msg(format!("{}. Service exists or try as admin.", e).as_str());
    } else {
        show_msg::show_msg("Service installed successfully");
    }
}

#[cfg(not(windows))]
fn main() {
    panic!("Only windows supported!");
}

#[cfg(windows)]
fn install_service() -> Result<(), String> {
    use std::ffi::OsString;
    use windows_service::{
        service::{ServiceAccess, ServiceErrorControl, ServiceInfo, ServiceStartType, ServiceType},
        service_manager::{ServiceManager, ServiceManagerAccess},
    };

    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(
        None::<&str>, manager_access
    ).map_err(|e| {
        format!("Error accessing service manager ({})", e)
    })?;

    let service_binary_path = ::std::env::current_exe()
        .unwrap()
        .with_file_name("rs_google_photos_sync.exe");

    let service_info = ServiceInfo {
        name: OsString::from("rs_google_photos_sync"),
        display_name: OsString::from("Sync-down Google Photos service"),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec![OsString::from("--winservice")],
        dependencies: vec![],
        account_name: None, // run as System
        account_password: None,
    };

    let _ = service_manager.create_service(
        service_info, ServiceAccess::empty()
    ).map_err(|e| {
        format!("Error creating service ({})", e)
    })?;

    Ok(())
}
