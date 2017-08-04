//! System-dependent functions, or anything that this library is unable to
//! do without help from the OS.

use std::ffi::OsStr;
use std::path::Path;

extern crate libc;

#[cfg(target_os = "redox")]
extern crate syscall;


#[cfg(any(target_os = "macos", target_os = "ios"))]
extern {
    fn gettimeofday(tp: *mut libc::timeval, tzp: *mut libc::timezone) -> libc::c_int;
}

#[cfg(all(unix, not(target_os = "macos"), not(target_os = "ios"), not(target_os = "redox")))]
extern {
    fn clock_gettime(clk_id: libc::c_int, tp: *mut libc::timespec) -> libc::c_int;
}


/// Returns the system’s current time, as a tuple of seconds elapsed since
/// the Unix epoch, and the millisecond of the second.
#[cfg(any(target_os = "macos", target_os = "ios"))]
pub unsafe fn sys_time() -> (i64, i16) {
    use std::ptr::null_mut;

    let mut tv = libc::timeval { tv_sec: 0, tv_usec: 0 };
    let _ = gettimeofday(&mut tv, null_mut());
    (tv.tv_sec, (tv.tv_usec / 1000) as i16)
}

/// Returns the system’s current time, as a tuple of seconds elapsed since
/// the Unix epoch, and the millisecond of the second.
#[cfg(not(target_os = "redox"))]
#[cfg(not(any(target_os = "macos", target_os = "ios", windows)))]
pub unsafe fn sys_time() -> (i64, i16) {
    let mut tv = libc::timespec { tv_sec: 0, tv_nsec: 0 };
    clock_gettime(libc::CLOCK_REALTIME, &mut tv);
    (tv.tv_sec as i64, (tv.tv_nsec / 1000) as i16)
}

/// Returns the system’s current time, as a tuple of seconds elapsed since
/// the Unix epoch, and the millisecond of the second.
#[cfg(target_os = "redox")]
pub unsafe fn sys_time() -> (i64, i16) {
   let mut ts = syscall::TimeSpec::default();
   let _ = syscall::clock_gettime(syscall::CLOCK_REALTIME, &mut ts);
   (ts.tv_sec, (ts.tv_nsec / 1000) as i16)
}

/// Attempts to determine the system’s current time zone. There’s no
/// guaranteed way to do this, so this function returns `None` if no
/// timezone could be found.
pub fn sys_timezone() -> Option<String> {
    use std::fs::read_link;

    let link = match read_link("/etc/localtime") {
        Ok(link) => link,
        Err(_) => return None,
    };

    if let Some(tz) = extract_timezone(&*link) {
        if !tz.is_empty() {
            return Some(tz);
        }
    }

    None
}

/// Given a path, returns whether a valid zoneinfo timezone name can be
/// detected at the end of that path.
fn extract_timezone(path: &Path) -> Option<String> {
    let mut bits = Vec::new();

    for pathlet in path.iter().rev().take_while(|c| is_tz_component(c)) {
        match pathlet.to_str() {
            Some(s) => bits.insert(0, s),
            None => return None,
        }
    }

    Some(bits.join("/"))
}

/// Returns whether the input string could be used as a component of a
/// zoneinfo timezone name, which in this case is whether its first
/// character is a capital letter.
fn is_tz_component(component: &OsStr) -> bool {
    if let Some(component_str) = component.to_str() {
        let first_char = component_str.chars().next().unwrap();
        first_char.is_uppercase()
    }
    else {
        false
    }
}


#[cfg(test)]
mod test {
    use super::{sys_time, extract_timezone};
    use std::path::Path;

    #[test]
    fn sanity_check() {
        assert!((0, 0) != unsafe { sys_time() })
    }

    #[test]
    fn two() {
        let timezone = extract_timezone(Path::new("/usr/share/zoneinfo/Europe/London"));
        assert_eq!(timezone, Some("Europe/London".to_string()));
    }

    #[test]
    fn one() {
        let timezone = extract_timezone(Path::new("/usr/share/zoneinfo/CST6CDT"));
        assert_eq!(timezone, Some("CST6CDT".to_string()));
    }
}
