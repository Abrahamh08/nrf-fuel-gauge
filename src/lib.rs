#![no_std]

use core::{ffi::CStr, marker::PhantomData, time::Duration};
use nrfxlib_sys as sys;

#[derive(Copy, Clone, Debug)]
pub enum Error { InvalidParam, BufferTooSmall, Other(i32) }

#[inline]
fn cvt_rc(rc: i32) -> Result<(), Error> {
    match rc { 0 => Ok(()), -22 => Err(Error::InvalidParam), x => Err(Error::Other(x)) }
}

pub fn version() -> &'static str {
    unsafe {
        if sys::nrf_fuel_gauge_version.is_null() { "" }
        else { CStr::from_ptr(sys::nrf_fuel_gauge_version).to_str().unwrap_or("") }
    }
}

pub fn build_date() -> &'static str {
    unsafe {
        if sys::nrf_fuel_gauge_build_date.is_null() { "" }
        else { CStr::from_ptr(sys::nrf_fuel_gauge_build_date).to_str().unwrap_or("") }
    }
}

#[derive(Copy, Clone)]
pub struct Initial { pub v0: f32, pub i0: f32, pub t0: f32 }

pub type RawConfig = sys::nrf_fuel_gauge_config_parameters;

pub fn default_config() -> RawConfig {
    let mut c = core::mem::MaybeUninit::<RawConfig>::zeroed();
    unsafe { sys::nrf_fuel_gauge_opt_params_default_get(c.as_mut_ptr()); }
    unsafe { c.assume_init() }
}

#[derive(Copy, Clone, Default)]
pub struct Runtime {
    pub a: Option<f32>, pub b: Option<f32>, pub c: Option<f32>, pub d: Option<f32>,
    pub discard_positive_deltaz: bool,
}
impl From<Runtime> for sys::nrf_fuel_gauge_runtime_parameters {
    fn from(r: Runtime) -> Self {
        fn or_nan(x: Option<f32>) -> f32 { x.unwrap_or(f32::NAN) }
        sys::nrf_fuel_gauge_runtime_parameters {
            a: or_nan(r.a), b: or_nan(r.b), c: or_nan(r.c), d: or_nan(r.d),
            discard_positive_deltaz: r.discard_positive_deltaz,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct StateInfo { pub yhat: f32, pub r0: f32, pub t_truncated: f32 }
impl From<sys::nrf_fuel_gauge_state_info> for StateInfo {
    fn from(s: sys::nrf_fuel_gauge_state_info) -> Self {
        Self { yhat: s.yhat, r0: s.r0, t_truncated: s.T_truncated }
    }
}

pub struct FuelGauge<'m> { _pin: PhantomData<&'m ()> }

impl<'m> FuelGauge<'m> {
    pub fn init(
        model: &'m sys::battery_model,
        init: Initial,
        config: Option<&RawConfig>,
    ) -> Result<Self, Error> {
        let mut params = sys::nrf_fuel_gauge_init_parameters {
            v0: init.v0, i0: init.i0, t0: init.t0,
            model: model as *const sys::battery_model,
            opt_params: core::ptr::null(),
        };
        if let Some(cfg) = config {
            params.opt_params = cfg as *const _;
        }
        let rc = unsafe { sys::nrf_fuel_gauge_init(&params, core::ptr::null_mut()) };
        cvt_rc(rc).map(|_| Self { _pin: PhantomData })
    }

    pub fn update(&mut self, v: f32, i: f32, t: f32, dt: f32, vbus_present: bool) -> f32 {
        unsafe { sys::nrf_fuel_gauge_process(v, i, t, dt, vbus_present, core::ptr::null_mut()) }
    }

    pub fn update_with_info(&mut self, v: f32, i: f32, t: f32, dt: f32) -> (f32, StateInfo) {
        let mut s = sys::nrf_fuel_gauge_state_info { yhat:0.0, r0:0.0, T_truncated:0.0 };
        let soc = unsafe { sys::nrf_fuel_gauge_process(v, i, t, dt, true, &mut s) };
        (soc, StateInfo::from(s))
    }

    pub fn time_to_empty(&self) -> Option<Duration> {
        let s = unsafe { sys::nrf_fuel_gauge_tte_get() };
        if s.is_nan() || s.is_sign_negative() { None } else { Some(Duration::from_secs_f32(s)) }
    }

    pub fn time_to_full(&self, cc_charging: bool, i_term: f32) -> Option<Duration> {
        let s = unsafe { sys::nrf_fuel_gauge_ttf_get(cc_charging as _, i_term) };
        if s.is_nan() || s.is_sign_negative() { None } else { Some(Duration::from_secs_f32(s)) }
    }

    pub fn enter_idle(&mut self, v: f32, t: f32, i_avg: f32) {
        unsafe { sys::nrf_fuel_gauge_idle_set(v, t, i_avg) };
    }

    pub fn set_runtime(&mut self, r: Runtime) {
        let c: sys::nrf_fuel_gauge_runtime_parameters = r.into();
        unsafe { sys::nrf_fuel_gauge_param_adjust(&c as *const _); }
    }

    pub fn set_opt_params(&mut self, cfg: &RawConfig) {
        unsafe { sys::nrf_fuel_gauge_opt_params_adjust(cfg as *const _); }
    }
}
