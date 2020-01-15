use std::io::Error;

#[cfg(windows)]
pub fn show_msg(msg: &str) {
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
pub fn show_msg(msg: &str) {
    panic!("show msg not supported");
}