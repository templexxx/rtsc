use core::arch::x86_64::{CpuidResult, __cpuid};
use std::arch::asm;
use std::arch::x86_64::_rdtsc;
use std::mem::transmute;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
mod tests {
    use std::mem;

    // use crate::{enable_tsc, get_system_clock_source, has_invariant_tsc, UNIX_NANO, unix_nano_tsc};
    use crate::{
        get_system_clock_source, is_enabled, reset, unix_nano_std, unix_nano_tsc, GetUnixNano,
        OffsetCoeff, OFFSET_COEFF, UNIX_NANO,
    };

    #[test]
    fn it_works() {
        println!("{}", mem::align_of::<OffsetCoeff>());
        println!("{}", mem::size_of::<OffsetCoeff>());
    }
}

type GetUnixNano = fn() -> i64;

pub static mut UNIX_NANO: GetUnixNano = unix_nano_std;
static mut TSC_ENABLED: bool = false;

pub fn unix_nano() -> i64 {
    unsafe {
        return UNIX_NANO();
    }
}

const NANOS_PER_SEC: i64 = 1_000_000_000;

pub fn unix_nano_std() -> i64 {
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
    return dur.as_secs() as i64 * NANOS_PER_SEC + dur.subsec_nanos() as i64;
}

/// `init` init UNIX_NANO implementation. Invoke it before using.
/// Not thread-safe.
pub fn init() {
    unsafe {
        if !TSC_ENABLED {
            if enable_tsc() {
                UNIX_NANO = unix_nano_tsc;
                TSC_ENABLED = true;
            }
        }
    }
}

pub fn is_enabled() -> bool {
    unsafe {
        return TSC_ENABLED;
    }
}

#[repr(align(128))]
union OffsetCoeff {
    arr: [i8; 128],
} // 128bytes for X86 false sharing range.

static mut OFFSET_COEFF: OffsetCoeff = OffsetCoeff { arr: [1; 128] };

pub unsafe fn calibrate() {
    if !TSC_ENABLED {
        return;
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn unix_nano_tsc() -> i64 {
    return 0 as i64;
}

#[cfg(target_arch = "x86_64")]
pub fn unix_nano_tsc() -> i64 {
    let ret;
    unsafe {
        asm!(
        "rdtsc",
        "sal rdx, $32",
        "or  rax, rdx",
        "vcvtsi2sd xmm0, xmm0, rax",     // ftsc = float64(tsc)
        "vmovdqa xmm3, [{0}]",  // get coeff
        "vmovhlps xmm4, xmm3, xmm3",    // get offset
        "vfmadd132pd xmm3, xmm4, xmm0",  // X0 * X3 + X4 -> X3: ftsc * coeff + offset
        "vcvttsd2si {1}, xmm3",
        in(reg) &OFFSET_COEFF,
        out(reg) ret,
        options(nostack)
        );
    }
    return ret;
}

#[cfg(not(target_arch = "x86_64"))]
pub fn store_offset_coeff(offset: f64, coeff: f64) {}

#[cfg(target_arch = "x86_64")]
pub fn store_offset_coeff(offset: f64, coeff: f64) {
    unsafe {
        asm!(
        "vmovq xmm4, {0}",
        "vmovq xmm5, {1}",
        "vmovlhps xmm6, xmm5, xmm4",    // offset in high bits.
        "vmovdqa [{2}], xmm6",
        in(reg) offset,
        in(reg) coeff,
        in(reg) &OFFSET_COEFF,
        options(nostack)
        );
    }
}

#[cfg(not(target_arch = "x86_64"))]
pub fn load_offset_coeff() -> (f64, f64) {
    return (0 as f64, 0 as f64);
}

#[cfg(target_arch = "x86_64")]
pub fn load_offset_coeff() -> (f64, f64) {
    let offset;
    let coeff;
    unsafe {
        asm!(
        "vmovdqa xmm3, [{0}]",  // get coeff
        "vmovhlps xmm4, xmm3, xmm3",    // get offset
        "vmovq {1}, xmm4",
        "vmovq {2}, xmm3",
        in(reg) &OFFSET_COEFF,
        out(reg) offset,
        out(reg) coeff,
        options(readonly, nostack, pure)
        );
    }
    return (offset, coeff);
}

#[cfg(not(target_arch = "x86_64"))]
fn enable_tsc() -> bool {
    return false;
}

#[cfg(target_arch = "x86_64")]
fn enable_tsc() -> bool {
    return if !has_invariant_tsc() && (get_system_clock_source() != "tsc") {
        false
    } else {
        is_x86_feature_detected!("avx") && is_x86_feature_detected!("fma")
    };
}

#[cfg(not(target_os = "linux"))]
fn get_system_clock_source() -> String {
    return String::new();
}

const CLOCK_SRC_PATH: &str = "/sys/devices/system/clocksource/clocksource0/current_clocksource";

#[cfg(all(target_os = "linux"))]
fn get_system_clock_source() -> String {
    use std::fs;
    let t = fs::read_to_string(CLOCK_SRC_PATH);
    let src = match t {
        Ok(text) => text,
        Err(_) => String::new(),
    };

    if !src.is_empty() {
        if src.ends_with('\n') {
            let mut s = src;
            s.pop();
            return s;
        }
    }
    return String::new();
}

// This function was copied from https://github.com/gnzlbg/tsc/blob/master/src/lib.rs,
// Thanks for his contribution.
#[cfg(all(target_arch = "x86_64"))]
fn has_invariant_tsc() -> bool {
    // Obtain the largest basic CPUID leaf supported by the CPUID
    let CpuidResult {
        eax: max_basic_leaf,
        ..
    } = unsafe { __cpuid(0_u32) };

    // Earlier Intel 486 => too old to have an invariant TSC.
    if max_basic_leaf < 1 {
        return false;
    }

    // Obtain the largest extended CPUID leaf supported by the CPUID
    let CpuidResult {
        eax: max_extended_leaf,
        ..
    } = unsafe { __cpuid(0x8000_0000_u32) };

    // CPU doesn't have "Advanced Power Management Information" => too old to
    // have an invariant TSC.
    if max_extended_leaf < 7 {
        return false;
    }

    let CpuidResult { edx, .. } = unsafe { __cpuid(0x8000_0007_u32) };

    // Test CPUID.80000007H:EDX[8], if the bit is set, the CPU has an
    // invariant TSC
    return edx & (1 << 8) != 0;
}
