mod show_msg;

#[cfg(windows)]
fn main() {
    if let Err(e) = uninstall_service() {
        println!("{}", e);
        show_msg::show_msg(format!("{}. Service not there or run as admin.", e).as_str());
    } else {
        show_msg::show_msg("Uninstalled successfully.");
    }
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
