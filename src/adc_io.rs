#![no_std]

use core::ffi::c_void;
use core::mem;
use core::ptr;
use core::time::Duration;

use zephyr::{printk, raw};
use zephyr::raw::k_work_init;
use crate::raw::adc_dt_spec;

extern crate alloc;
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
    pub static adc_channels: *const adc_dt_spec;
    pub static adc_channels_len: usize;
}

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
    pub fn new() -> Self {
        let channel_count;

        unsafe {
            if adc_channels.is_null() {
                panic!("The adc_channels array from C is NULL!");
            }

            channel_count = adc_channels_len;

            // ADC channel setup
            for i in 0..channel_count {
                let adc_dt_spec_ptr = adc_channels.add(i);
                let adc_dt_spec_ref = &*adc_dt_spec_ptr;

                let err = raw::adc_channel_setup_dt(adc_dt_spec_ref);
                if err < 0 {
                    log::warn!("Could not setup channel #{} ({})\n", i, err);
                    panic!("ADC channel setup failed");
                }
            }
        }

        let mut adc = Adc {
            options: raw::adc_sequence_options {
                interval_us: 0,
                callback: Some(Self::hard_isr as unsafe extern "C" fn(*const raw::device, *const raw::adc_sequence, u16) -> raw::adc_action),
                user_data: ptr::null_mut(),
                extra_samplings: 0,
            },
            // channels: Vec::new(), // We no longer create an empty Vec
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
                sample: Vec::with_capacity(channel_count), // Vektörü sınırlandırdık
                done_cb: None,
                done_cb_isr: None,
                state: AdcAction::Continue,
                adc: ptr::null_mut(),
            },
        };

        unsafe {
            k_work_init(&mut adc.isr_context.work, Some(Self::soft_isr as unsafe extern "C" fn(*mut raw::k_work)));
        }

        adc.isr_context.adc = &mut adc as *mut Adc;
        adc.options.user_data = &mut adc.isr_context as *mut _ as *mut c_void;
        adc
    }

    pub fn read_async(&mut self, interval: Duration, handler: Option<fn(usize, i16)>) {
        self.options.interval_us = interval.as_micros() as u32;

        let mut sequence = raw::adc_sequence {
            options: &self.options as *const raw::adc_sequence_options,
            channels: unsafe { bit(self.get_channel(0).channel_id as u32) }, // Use get_channel
            buffer: &mut self.isr_context.buffer as *mut i16 as *mut c_void,
            buffer_size: mem::size_of::<i16>(),
            resolution: unsafe { self.get_channel(0).resolution }, // Use get_channel
            calibrate: false,
            oversampling: 0,
        };

        self.channel_index = 0;
        self.isr_context.done_cb = handler;
        self.isr_context.done_cb_isr = None;
        self.isr_context.state = AdcAction::Continue;

        let res = unsafe {
            raw::adc_read_async(
                self.get_channel(0).dev, // Use get_channel
                &sequence as *const raw::adc_sequence,
                ptr::null_mut(),
            )
        };
        if res != 0 {
            log::warn!("Failed to start async ADC read: {}\n", res);
            panic!("ADC async read failed");
        }
    }

    pub fn read_async_isr(&mut self, interval: Duration, handler: Option<fn(usize, i16)>) {
        self.options.interval_us = interval.as_micros() as u32;

        let mut sequence = raw::adc_sequence {
            options: &self.options as *const raw::adc_sequence_options,
            channels: unsafe { bit(self.get_channel(0).channel_id as u32) }, // Use get_channel
            buffer: &mut self.isr_context.buffer as *mut i16 as *mut c_void,
            buffer_size: mem::size_of::<i16>(),
            resolution: unsafe { self.get_channel(0).resolution }, // Use get_channel
            calibrate: false,
            oversampling: 0,
        };

        self.channel_index = 0;
        self.isr_context.done_cb = None;
        self.isr_context.done_cb_isr = handler;
        self.isr_context.state = AdcAction::Continue;

        let res = unsafe {
            raw::adc_read_async(
                self.get_channel(0).dev, // Use get_channel
                &sequence as *const raw::adc_sequence,
                ptr::null_mut(),
            )
        };
        if res != 0 {
            log::warn!("Failed to start async ADC read in ISR: {}\n", res);
            panic!("ADC async read ISR failed");
        }
    }

    pub fn cancel_read(&mut self) {
        self.isr_context.state = AdcAction::Finish;
    }

    pub fn get_voltage(&self, idx: usize) -> i32 {
        let mut val_mv = self.isr_context.sample[idx] as i32;
        unsafe {
            raw::adc_raw_to_millivolts_dt(self.get_channel(idx), &mut val_mv) // Use get_channel
        };
        val_mv
    }

    pub fn get_value(&self, idx: usize) -> i32 {
        self.isr_context.sample[idx] as i32
    }

    extern "C" fn soft_isr(work: *mut raw::k_work) {
        if work.is_null() {
            return;
        }
        let context_ptr = unsafe { container_of(work, |c: &mut IsrContext| &mut c.work) };
        let context = unsafe { &mut *context_ptr };
        let adc = unsafe { &mut *context.adc };

        if adc.channel_count > 1 {
            let mut sequence = raw::adc_sequence {
                options: &adc.options as *const raw::adc_sequence_options,
                channels: unsafe { bit(adc.get_channel(adc.channel_index).channel_id as u32) }, // Use get_channel
                buffer: &mut context.buffer as *mut i16 as *mut c_void,
                buffer_size: mem::size_of::<i16>(),
                resolution: unsafe { adc.get_channel(adc.channel_index).resolution }, // Use get_channel
                calibrate: false,
                oversampling: 0,
            };
            let res = unsafe {
                raw::adc_read_async(
                    adc.get_channel(adc.channel_index).dev, // Use get_channel
                    &sequence as *const raw::adc_sequence,
                    ptr::null_mut(),
                )
            };
            if res != 0 {
                log::warn!("Failed to read async in soft ISR: {}\n", res);
                return;
            }
        }

        if let Some(cb) = context.done_cb {
            let idx = if adc.channel_index == 0 {
                adc.channel_count - 1
            } else {
                adc.channel_index - 1
            };
            cb(idx, context.sample[idx]);
        }
    }

    extern "C" fn hard_isr(
        _dev: *const raw::device,
        seq: *const raw::adc_sequence,
        _sampling_index: u16,
    ) -> raw::adc_action {
        if seq.is_null() || unsafe { (*seq).options.is_null() } {
            return AdcAction::Finish as raw::adc_action;
        }
        let context = unsafe { &mut *((*(*seq).options).user_data as *mut IsrContext) };
        let adc = unsafe { &mut *context.adc };

        context.sample[adc.channel_index] = unsafe { *( (*seq).buffer as *const i16) };

        if context.state != AdcAction::Finish {
            if adc.channel_count == 1 {
                context.state = AdcAction::Repeat;
                if let Some(cb) = context.done_cb_isr {
                    cb(0, context.sample[0]);
                } else {
                    unsafe { raw::k_work_submit(&mut context.work); }
                }
            } else if adc.channel_index + 1 < adc.channel_count {
                adc.channel_index += 1;
                context.state = AdcAction::Continue;
                if let Some(cb) = context.done_cb_isr {
                    cb(adc.channel_index - 1, context.sample[adc.channel_index - 1]);
                } else {
                    unsafe { raw::k_work_submit(&mut context.work); }
                }
            } else {
                if context.state != AdcAction::Repeat {
                    context.state = AdcAction::Repeat;
                    adc.channel_index -= 1;
                } else {
                    context.state = AdcAction::Continue;
                    adc.channel_index = 0;
                    if let Some(cb) = context.done_cb_isr {
                        cb(adc.channel_count - 1, context.sample[adc.channel_count - 1]);
                    } else {
                        unsafe { raw::k_work_submit(&mut context.work); }
                    }
                }
            }
        }
        context.state as raw::adc_action
    }

    // Helper function: Get the channel directly from adc_channels
    unsafe fn get_channel(&self, index: usize) -> &raw::adc_dt_spec {
        if index >= self.channel_count {
            panic!("Channel index out of bounds: {}", index);
        }
        &*adc_channels.add(index)
    }
}

unsafe fn container_of<T>(
    ptr: *mut raw::k_work,
    f: fn(&mut T) -> &mut raw::k_work,
) -> *mut T {
    let offset = {
        let mut dummy = mem::MaybeUninit::<T>::uninit();
        let dummy_ref: &mut T = &mut *dummy.as_mut_ptr();
        let dummy_ptr = f(dummy_ref) as *const raw::k_work;
        let field_ptr = ptr as *const raw::k_work;
        (field_ptr as *mut u8).offset(-(dummy_ptr as isize)) as usize
    };
    (ptr as *mut u8).offset(-(offset as isize)) as *mut T
}