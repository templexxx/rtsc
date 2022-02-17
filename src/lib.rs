use std::time::{SystemTime, UNIX_EPOCH};
use core::arch::x86_64::{CpuidResult, __cpuid};
use std::ptr::null;

#[cfg(test)]
mod tests {
    use crate::{unix_nano, unix_nano_tsc};

    #[test]
    fn it_works() {
        let result = 2 + 2;
        assert_eq!(result, 4);
        let a = unix_nano();
        let unix_nano = unix_nano_tsc;
        let b = unix_nano();
        println!("{} {} {}",b-a, a, b);
    }
}

const NANOS_PER_SEC: i64 = 1_000_000_000;

pub fn unix_nano() ->i64 {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    return  dur.as_secs() as i64 * NANOS_PER_SEC + dur.subsec_nanos() as i64
}

pub fn reset() {

}

pub fn unix_nano_tsc() -> i64 {
    return 0;
}

#[cfg(not(target_arch = "x86_64"))]
fn enable_tsc() -> bool {
    return false
}

#[cfg(target_arch = "x86_64")]
fn enable_tsc() -> bool {
    if !has_invariant_tsc() || (get_system_clock_source() != "tsc") {
        return false
    } else {

        return is_x86_feature_detected!("avx") && is_x86_feature_detected!("fma")
    }
}

#[cfg(not(target_os = "linux"))]
fn get_system_clock_source() -> String {
    return ""
}

#[cfg(target_os = "linux")]
fn get_system_clock_source() -> String {
    use std::fs;
    let t = fs::read_to_string("/sys/devices/system/clocksource/clocksource0/current_clocksource");
   let src =  match t {
       Ok(text) => text,
       Err(_) => String::new(),
   };

    if src.is_empty(){
        let mut s = src;
        s.pop();
        return s;
    }
   return String::new();
}

// This function was copied from https://github.com/gnzlbg/tsc/blob/master/src/lib.rs,
// Thanks for his contribution.
#[cfg(all(target_arch = "x86_64"))]
fn has_invariant_tsc() -> bool {
    // Obtain the largest basic CPUID leaf supported by the CPUID
    let CpuidResult { eax: max_basic_leaf, .. } = unsafe { __cpuid(0_u32) };

    // Earlier Intel 486 => too old to have an invariant TSC.
    if max_basic_leaf < 1 {
        return false;
    }

    // Obtain the largest extended CPUID leaf supported by the CPUID
    let CpuidResult { eax: max_extended_leaf, .. } =
        unsafe { __cpuid(0x8000_0000_u32) };

    // CPU doesn't have "Advanced Power Management Information" => too old to
    // have an invariant TSC.
    if max_extended_leaf < 7 {
        return false;
    }

    let CpuidResult { edx, .. } = unsafe { __cpuid(0x8000_0007_u32) };

    // Test CPUID.80000007H:EDX[8], if the bit is set, the CPU has an
    // invariant TSC
   return  edx & (1 << 8) != 0
}
