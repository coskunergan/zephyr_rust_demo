// Copyright (c) 2024 Linaro LTD
// SPDX-License-Identifier: Apache-2.0

#![no_std]
// Sigh. The check config system requires that the compiler be told what possible config values
// there might be.  This is completely impossible with both Kconfig and the DT configs, since the
// whole point is that we likely need to check for configs that aren't otherwise present in the
// build.  So, this is just always necessary.
//#![allow(unexpected_cfgs)]

use log::warn;
extern crate alloc;
//use zephyr::time::{sleep, Duration};

use embassy_time::{Duration, Ticker};

use zephyr::{
    //device::gpio::{GpioPin, GpioToken},
    embassy::Executor,
    raw::{GPIO_INPUT, GPIO_OUTPUT_ACTIVE, GPIO_PULL_DOWN},
};

use embassy_executor::Spawner;
use static_cell::StaticCell;

static EXECUTOR_MAIN: StaticCell<Executor> = StaticCell::new();

#[no_mangle]
extern "C" fn rust_main() {
    unsafe {
        zephyr::set_logger().unwrap();
    }

    warn!("Starting blinky");

    let executor = EXECUTOR_MAIN.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(main(spawner)).unwrap();
    })
}

#[embassy_executor::task]
async fn main(spawner: Spawner) {
    warn!("Inside of blinky");

    let _ = spawner;

    //let mut led_red = zephyr::devicetree::aliases::led0::get_instance().unwrap();
    let mut led = zephyr::devicetree::labels::led::get_instance().unwrap();
    let mut button = zephyr::devicetree::labels::button::get_instance().unwrap();
    let mut gpio_token = unsafe { zephyr::device::gpio::GpioToken::get_instance().unwrap() };

    if !led.is_ready() {
        warn!("LED is not ready");
        loop {}
    }

    unsafe {
        led.configure(&mut gpio_token, GPIO_OUTPUT_ACTIVE);
        button.configure(&mut gpio_token, GPIO_INPUT | GPIO_PULL_DOWN);
    }
    //let duration = Duration::millis_at_least(500);
    loop {
        unsafe {
            led.toggle_pin(&mut gpio_token);
        }

        //unsafe { button.wait_for_low(&mut gpio_token).await };

        if unsafe { button.get(&mut gpio_token) } == true {
            //sleep(duration / 10);
            let mut ticker = Ticker::every(Duration::from_millis(50));
            ticker.next().await;
        } else {
            //sleep(duration);
            let mut ticker = Ticker::every(Duration::from_millis(500));
            ticker.next().await;
        }
    }
}
