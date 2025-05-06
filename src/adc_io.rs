#![no_std]

use core::ffi::c_void;
use core::mem;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};
use core::time::Duration;

use zephyr::{raw};
use zephyr::raw::{adc_dt_spec, k_work_init, k_work_submit};

extern crate alloc;
use alloc::vec;
use alloc::vec::Vec;

#[repr(C)]
#[derive(PartialEq, Clone, Copy)]
enum AdcAction {
    Continue = 0,
    Repeat = 1,
    Finish = 2,
}

const fn bit(n: u32) -> u32 {
    1u32 << n
}

extern "C" {
    fn get_adc_dt_spec() -> *const adc_dt_spec;
    fn get_adc_dt_len() -> usize;
}

// Statik başlatma bayrağı
static CHANNELS_INITIALIZED: AtomicBool = AtomicBool::new(false);

pub struct Adc {
    options: raw::adc_sequence_options,
    channel_count: usize,
    channel_index: usize,
    isr_context: IsrContext,
}

struct IsrContext {
    work: raw::k_work,
    buffer: i16,
    sample: Vec<i16>,
    done_cb: Option<fn(usize, i16)>,
    done_cb_isr: Option<fn(usize, i16)>,
    state: AdcAction,
    adc: *mut Adc,
}

impl Adc {
    /// ADC kanallarını yalnızca bir kez başlatır (sadece 0. indeks)
    fn init_channels() {
        if !CHANNELS_INITIALIZED.swap(true, Ordering::SeqCst) {
            // adc_channels işaretçisini al
            let adc_channels = unsafe { get_adc_dt_spec() };
            if adc_channels.is_null() {
                log::info!("adc_channels is null");
                panic!("adc_channels is null");
            }

            // Kanal sayısını al
            let adc_channels_len = unsafe { get_adc_dt_len() };
            if adc_channels_len == 0 {
                log::info!("adc_channels_len is 0");
                panic!("adc_channels_len is 0");
            }

            // Devicetree’deki kanal sayısını ve işaretçiyi logla
            log::info!("Channel count: {}", adc_channels_len);
            log::info!("adc_channels ptr: {:p}", adc_channels);

            // Yalnızca 0. indeksi yapılandır
            let adc_dt_spec_ptr = unsafe { adc_channels.add(0) };
            let adc_dt_spec_ref = unsafe { &*adc_dt_spec_ptr };
            // adc_dt_spec içeriğini detaylı logla
            log::info!("adc_dt_spec[0]: dev={:p}, channel_id={}, resolution={}, oversampling={}",
                       adc_dt_spec_ref.dev,
                       adc_dt_spec_ref.channel_id,
                       adc_dt_spec_ref.resolution,
                       adc_dt_spec_ref.oversampling);
            // channel_cfg içeriğini logla
            log::info!("adc_dt_spec[0].channel_cfg: gain={}, reference={}, acquisition_time={}, differential={}",
                       adc_dt_spec_ref.channel_cfg.gain,
                       adc_dt_spec_ref.channel_cfg.reference,
                       adc_dt_spec_ref.channel_cfg.acquisition_time,
                       adc_dt_spec_ref.channel_cfg._bitfield_1.get_bit(0) as u32);

            if adc_dt_spec_ref.dev.is_null() {
                log::info!("ADC device pointer is null for channel 0");
                panic!("ADC device pointer is null");
            }

            let device = unsafe { &*adc_dt_spec_ref.dev };
            // Cihaz adını güvenli bir şekilde logla
            let device_name = unsafe { core::ffi::CStr::from_ptr((*device).name) }
                .to_str()
                .unwrap_or("unknown");
            log::info!("Checking device: {}", device_name);
            let ready = unsafe { raw::device_is_ready(device) };
            if !ready {
                log::info!("ADC device {} is not ready", device_name);
                panic!("ADC device not ready");
            }

            log::info!("Configuring channel 0");
            let err = unsafe { raw::adc_channel_setup_dt(adc_dt_spec_ref) };
            if err < 0 {
                log::info!("Could not setup channel 0, err: {}", err);
                panic!("ADC channel setup failed");
            }
            log::info!("Channel 0 configured");
        }
    }

    pub fn new() -> Self {
        // Kanal kurulumunu bir kez yap
        Self::init_channels();

        // Yalnızca bir kanal kullanıldığı için channel_count 1
        let channel_count = 1;

        let mut adc = Adc {
            options: raw::adc_sequence_options {
                interval_us: 0,
                callback: Some(Self::hard_isr),
                user_data: ptr::null_mut(),
                extra_samplings: 0,
            },
            channel_count,
            channel_index: 0,
            isr_context: IsrContext {
                work: raw::k_work {
                    node: raw::sys_snode_t {
                        next: ptr::null_mut(),
                    },
                    handler: None,
                    queue: ptr::null_mut(),
                    flags: 0,
                },
                buffer: 0,
                sample: vec![0; channel_count], // Sadece 1 kanal için vektör
                done_cb: None,
                done_cb_isr: None,
                state: AdcAction::Continue,
                adc: ptr::null_mut(),
            },
        };

        unsafe {
            k_work_init(&mut adc.isr_context.work, Some(Self::soft_isr));
        }

        adc.isr_context.adc = &mut adc as *mut Adc;
        adc.options.user_data = &mut adc.isr_context as *mut _ as *mut c_void;
        adc
    }

    pub fn read_async(&mut self, interval: Duration, handler: Option<fn(usize, i16)>) {
        self.options.interval_us = interval.as_micros() as u32;

        let channel = unsafe { get_channel(0, self.channel_count) };
        let sequence = raw::adc_sequence {
            options: &self.options as *const raw::adc_sequence_options,
            channels: bit(channel.channel_cfg.channel_id() as u32),
            buffer: &mut self.isr_context.buffer as *mut i16 as *mut c_void,
            buffer_size: mem::size_of::<i16>(),
            resolution: channel.resolution,
            calibrate: false,
            oversampling: 0,
        };

        self.channel_index = 0;
        self.isr_context.done_cb = handler;
        self.isr_context.done_cb_isr = None;
        self.isr_context.state = AdcAction::Continue;

        let res = unsafe {
            raw::adc_read_async(
                channel.dev,
                &sequence as *const raw::adc_sequence,
                ptr::null_mut(),
            )
        };
        if res != 0 {
            log::info!("Failed to start async ADC read: {}", res);
            panic!("ADC async read failed");
        }
    }

    pub fn read_async_isr(&mut self, interval: Duration, handler: Option<fn(usize, i16)>) {
        self.options.interval_us = interval.as_micros() as u32;

        let channel = unsafe { get_channel(0, self.channel_count) };
        let sequence = raw::adc_sequence {
            options: &self.options as *const raw::adc_sequence_options,
            channels: bit(channel.channel_cfg.channel_id() as u32),
            buffer: &mut self.isr_context.buffer as *mut i16 as *mut c_void,
            buffer_size: mem::size_of::<i16>(),
            resolution: channel.resolution,
            calibrate: false,
            oversampling: 0,
        };

        self.channel_index = 0;
        self.isr_context.done_cb = None;
        self.isr_context.done_cb_isr = handler;
        self.isr_context.state = AdcAction::Continue;

        let res = unsafe {
            raw::adc_read_async(
                channel.dev,
                &sequence as *const raw::adc_sequence,
                ptr::null_mut(),
            )
        };
        if res != 0 {
            log::info!("Failed to start async ADC read in ISR: {}", res);
            panic!("ADC async read ISR failed");
        }
    }

    pub fn cancel_read(&mut self) {
        self.isr_context.state = AdcAction::Finish;
    }

    pub fn get_voltage(&self, idx: usize) -> i32 {
        if idx >= self.isr_context.sample.len() {
            panic!("Index out of bounds: {}", idx);
        }
        let channel = unsafe { get_channel(idx, self.channel_count) };
        let mut val_mv = if channel.channel_cfg._bitfield_1.get_bit(0) { // differential bit
            self.isr_context.sample[idx] as i16 as i32
        } else {
            self.isr_context.sample[idx] as i32
        };
        unsafe {
            raw::adc_raw_to_millivolts_dt(channel, &mut val_mv);
        }
        val_mv
    }

    pub fn get_value(&self, idx: usize) -> i32 {
        if idx >= self.isr_context.sample.len() {
            panic!("Index out of bounds: {}", idx);
        }
        let channel = unsafe { get_channel(idx, self.channel_count) };
        if channel.channel_cfg._bitfield_1.get_bit(0) { // differential bit
            self.isr_context.sample[idx] as i16 as i32
        } else {
            self.isr_context.sample[idx] as i32
        }
    }

    extern "C" fn soft_isr(work: *mut raw::k_work) {
        if work.is_null() {
            return;
        }
        let context_ptr = unsafe { container_of(work, |c: &mut IsrContext| &mut c.work) };
        let context = unsafe { &mut *context_ptr };
        let adc = unsafe { &mut *context.adc };

        // Tek kanal kullanıldığı için ek okuma gerekmez
        if let Some(cb) = context.done_cb {
            let idx = 0; // Sadece kanal 0
            if idx < context.sample.len() {
                cb(idx, context.sample[idx]);
            }
        }
    }

    extern "C" fn hard_isr(
        _dev: *const raw::device,
        seq: *const raw::adc_sequence,
        _sampling_index: u16,
    ) -> raw::adc_action {
        if seq.is_null() || unsafe { (*seq).options.is_null() } || unsafe { (*seq).buffer.is_null() } {
            return AdcAction::Finish as raw::adc_action;
        }
        let context = unsafe { &mut *((*(*seq).options).user_data as *mut IsrContext) };
        let adc = unsafe { &mut *context.adc };

        if adc.channel_index < context.sample.len() {
            context.sample[adc.channel_index] = unsafe { *((*seq).buffer as *const i16) };
        } else {
            log::info!("Invalid channel_index: {}", adc.channel_index);
            return AdcAction::Finish as raw::adc_action;
        }

        if context.state != AdcAction::Finish {
            context.state = AdcAction::Repeat;
            if let Some(cb) = context.done_cb_isr {
                cb(0, context.sample[0]);
            } else {
                unsafe { k_work_submit(&mut context.work); }
            }
        }
        context.state as raw::adc_action
    }
}

unsafe fn get_channel(index: usize, channel_count: usize) -> &'static raw::adc_dt_spec {
    if index >= channel_count {
        panic!("Channel index out of bounds: {}", index);
    }
    let adc_channels = get_adc_dt_spec();
    if adc_channels.is_null() {
        panic!("adc_channels is null");
    }
    &*adc_channels.add(index)
}

unsafe fn container_of<T>(
    ptr: *mut raw::k_work,
    f: fn(&mut T) -> &mut raw::k_work,
) -> *mut T {
    if ptr.is_null() {
        panic!("container_of: null pointer");
    }
    let offset = {
        let mut dummy = mem::MaybeUninit::<T>::uninit();
        let dummy_ref: &mut T = &mut *dummy.as_mut_ptr();
        let dummy_ptr = f(dummy_ref) as *const raw::k_work;
        let field_ptr = ptr as *const raw::k_work;
        (field_ptr as *mut u8).offset(-(dummy_ptr as isize)) as usize
    };
    (ptr as *mut u8).offset(-(offset as isize)) as *mut T
}