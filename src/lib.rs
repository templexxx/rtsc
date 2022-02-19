use core::arch::x86_64::{CpuidResult, __cpuid};
use std::arch::asm;
use std::arch::x86_64::_rdtsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};


type GetUnixNano = fn() -> i64;

static mut UNIX_NANO: GetUnixNano = unix_nano_std;
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

/// `init` init UNIX_NANO implementation. Invoke it before using this lib.
/// Not thread-safe.
pub fn init() {
    unsafe {
        if !TSC_ENABLED {
            if enable_tsc() {
                UNIX_NANO = unix_nano_tsc;
                do_calibrate();
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

/// unix_nano_timestamp = tsc_register_value * Coeff(coefficient) + Offset(offset to system clock).
/// Coeff = 1 / (tsc_frequency / 1e9).
/// We could regard coeff as the inverse of TSCFrequency(GHz) (actually it just has mathematics property)
/// for avoiding future dividing.
/// MUL gets much better performance than DIV.
static mut OFFSET_COEFF: OffsetCoeff = OffsetCoeff { arr: [1; 128] };

#[cfg(not(target_arch = "x86_64"))]
pub fn calibrate() {
    return;
}

// 128 is enough for calculating frequency & offset supporting 5-10 minutes high accurate clock.
const SAMPLES: usize = 128;
const SAMPLE_DURATION: Duration = Duration::from_millis(16);
// 256 is enough for finding the lowest sys clock cost in most cases.
// Although time.Now() is using VDSO to get time, but it's unstable,
// sometimes it will take more than 1000ns,
// we have to use a big loop(e.g. 256) to get the "real" clock.
// And it won't take a long time to finish calibrating job, only about 20µs.
// [tscClock, wc, tscClock, wc, ..., tscClock]
const GET_CLOSEST_TSCSYS_RETRIES: usize = 256;

pub const DEFAULT_CALIBRATE_INTERVAL: Duration = Duration::from_secs(300);

/// `calibrate` calibrates tsc clock.
///
/// It's a good practice that run Calibrate periodically (e.g., 5 min is a good start because the auto NTP adjust is always every 11 min).
#[cfg(target_arch = "x86_64")]
pub fn calibrate() {
    unsafe {
        if !TSC_ENABLED {
            return;
        }
    }

    do_calibrate();
}

fn do_calibrate() {
    let cnt = SAMPLES;

    let mut tscs: Vec<f64> = Vec::with_capacity(cnt * 2);
    let mut syss: Vec<f64> = Vec::with_capacity(cnt * 2);

    let mut i = 0;
    while i < cnt {
        let (tsc0, sys0) = get_closest_tsc_sys(GET_CLOSEST_TSCSYS_RETRIES);
        thread::sleep(SAMPLE_DURATION);
        let (tsc1, sys1) = get_closest_tsc_sys(GET_CLOSEST_TSCSYS_RETRIES);

        tscs.push(tsc0 as f64);
        tscs.push(tsc1 as f64);

        syss.push(sys0 as f64);
        syss.push(sys1 as f64);

        i += 1;
    }

    let (coeff, offset) = simple_linear_regression(tscs, syss);
    store_offset_coeff(offset, coeff);
}

fn is_even(n: usize) -> bool {
    return n & 1 == 0;
}

/// `get_closest_tsc_sys` tries to get the closest tsc register value nearby the system clock in a loop.
fn get_closest_tsc_sys(retries: usize) -> (i64, i64) {
    let cap = retries + retries + 1;
    let mut timeline: Vec<i64> = Vec::with_capacity(cap);

    unsafe {
        timeline.push(_rdtsc() as i64);
    }

    let mut i = 1;
    while i < cap - 1 {
        unsafe {
            timeline.push(unix_nano_std());
            timeline.push(_rdtsc() as i64);
        }
        i += 2;
    }

    // min_delta is the smallest gap between two adjacent tsc,
    // which means the smallest gap between sys clock and tsc clock too.
    let mut min_delta = i64::MAX;
    let mut min_index = 1;

    // clock's precision is only µs (on macOS),
    // which means we will get multi same sys clock in timeline,
    // and the middle one is closer to the real time in statistics.
    // Try to find the minimum delta when sys clock is in the "middle".
    let mut i = 1;
    while i < cap - 1 {
        let mut last = timeline[i];
        let mut j = i + 2;
        while j < cap - 1 {
            if timeline[j] != last {
                let mut mid = (i + j - 2) >> 1;
                if is_even(mid) {
                    mid += 1;
                }

                let delta = timeline[mid + 1] - timeline[mid - 1];
                if delta < min_delta {
                    min_delta = delta;
                    min_index = mid;
                }

                i = j;
                last = timeline[j];
            }

            j += 2;
        }
        i += 2;
    }

    let tsc_clock = (timeline[min_index + 1] + timeline[min_index - 1]) >> 1;
    let sys = timeline[min_index];

    return (tsc_clock, sys);
}

/// `simple_linear_regression` uses simple linear regression to calculate tsc register 1/frequency(coefficient) & offset.
fn simple_linear_regression(tscs: Vec<f64>, syss: Vec<f64>) -> (f64, f64) {
    let mut t_mean: f64 = 0.0;
    let mut s_mean: f64 = 0.0;

    for i in &tscs {
        t_mean += i;
    }
    t_mean = t_mean / tscs.len() as f64;
    for i in &syss {
        s_mean += i;
    }
    s_mean = s_mean / syss.len() as f64;

    let mut denominator: f64 = 0.0;
    let mut numerator: f64 = 0.0;
    for i in 0..tscs.len() {
        numerator += (tscs[i] - t_mean) * (syss[i] - s_mean);
        denominator += (tscs[i] - t_mean).powf(2.0);
    }

    let coeff = numerator / denominator;
    let offset = s_mean - (coeff * t_mean);

    return (coeff, offset);
}

#[cfg(not(target_arch = "x86_64"))]
pub fn unix_nano_tsc() -> i64 {
    return unix_nano_std();
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

#[cfg(target_arch = "x86_64")]
fn enable_tsc() -> bool {
    return if !has_invariant_tsc() && !is_system_clock_source_tsc() {
        false
    } else {
        is_x86_feature_detected!("avx") && is_x86_feature_detected!("fma")
    };
}

#[cfg(not(target_os = "linux"))]
fn is_system_clock_source_tsc() -> bool {
    return false;
}

const CLOCK_SRC_PATH: &str = "/sys/devices/system/clocksource/clocksource0/current_clocksource";

#[cfg(all(target_os = "linux"))]
fn is_system_clock_source_tsc() -> bool {
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
            return s == "tsc";
        }
    }
    return false;
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
