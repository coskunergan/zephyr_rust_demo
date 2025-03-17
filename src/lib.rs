// Copyright (c) 2024 Linaro LTD
// SPDX-License-Identifier: Apache-2.0

#![no_std]
// Sigh. The check config system requires that the compiler be told what possible config values
// there might be.  This is completely impossible with both Kconfig and the DT configs, since the
// whole point is that we likely need to check for configs that aren't otherwise present in the
// build.  So, this is just always necessary.
#![allow(unexpected_cfgs)]

use log::warn;

use zephyr::time::{sleep, Duration};

use zephyr::{
    raw::{GPIO_INPUT, GPIO_OUTPUT_ACTIVE, GPIO_PULL_DOWN},
};

#[no_mangle]
extern "C" fn rust_main() {
    unsafe {
        zephyr::set_logger().unwrap();
    }

    warn!("Starting blinky");

    do_blink();
}

#[cfg(dt = "aliases::led0")]
fn do_blink() {
    warn!("Inside of blinky");

    let mut led0 = zephyr::devicetree::aliases::led0::get_instance().unwrap();
    let mut button = zephyr::devicetree::labels::button::get_instance().unwrap();
    let mut gpio_token = unsafe { zephyr::device::gpio::GpioToken::get_instance().unwrap() };

    if !led0.is_ready() {
        warn!("LED is not ready");
        loop {}
    }

    unsafe {
        led0.configure(&mut gpio_token, GPIO_OUTPUT_ACTIVE);
        button.configure(&mut gpio_token, GPIO_INPUT | GPIO_PULL_DOWN);
    }
    let duration = Duration::millis_at_least(500);
    loop {
        unsafe {
            led0.toggle_pin(&mut gpio_token);
        }

        if unsafe { button.get(&mut gpio_token) } == true {
           sleep(duration / 10);
        } else {
            sleep(duration);
        }
        
    }
}

#[cfg(not(dt = "aliases::led0"))]
fn do_blink() {
    warn!("No leds configured");
    loop {}
}
