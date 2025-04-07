// Copyright (c) 2024 Linaro LTD
// SPDX-License-Identifier: Apache-2.0

#![no_std]
// Sigh. The check config system requires that the compiler be told what possible config values
// there might be.  This is completely impossible with both Kconfig and the DT configs, since the
// whole point is that we likely need to check for configs that aren't otherwise present in the
// build.  So, this is just always necessary.

//#![allow(unexpected_cfgs)]
//#![allow(warnings)]

//use log::warn;
extern crate alloc;

use embassy_time::{Duration, Timer};

use alloc::boxed::Box;

use zephyr::{
    device::gpio::{GpioPin, GpioToken},
    sync::{Arc, Mutex},
};

use embassy_executor::Executor;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use static_cell::StaticCell;

mod button;
mod led;

static EXECUTOR_MAIN: StaticCell<Executor> = StaticCell::new();

pub static BUTTON_SIGNAL: Signal<CriticalSectionRawMutex, bool> = Signal::new();

//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
#[no_mangle]
extern "C" fn rust_main() {
    unsafe {
        zephyr::set_logger().unwrap();
    }

    let executor = EXECUTOR_MAIN.init(Executor::new());
    executor.run(|spawner| {
        spawner.spawn(main(spawner)).unwrap();
    })
}
//////////////////////////////////////////////////////////////////////////////////
////////////////////////////////// MAIN //////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
#[embassy_executor::task]
async fn main(spawner: Spawner) {
    let gpio_token = Arc::new(Mutex::new(unsafe { GpioToken::get_instance().unwrap() }));
    log::info!("GPIO token olusturuldu");

    let led_green = zephyr::devicetree::labels::led::get_instance().unwrap();
    let led_red = zephyr::devicetree::labels::led_red::get_instance().unwrap();

    log::info!("LED pinleri alindi: led_green ve led_red");

    declare_leds!(
        spawner,
        gpio_token,
        [
            (led_green, Duration::from_millis(100)),
            (led_red, Duration::from_millis(500))
        ]
    );
    log::info!("LED'ler baslatildi");

    let button = zephyr::devicetree::labels::button::get_instance().unwrap();

    log::info!("Button Pini Alindi.");

    declare_buttons!(
        spawner,
        gpio_token,
        [(
            button,
            || {
                log::info!("Butona Basildi!");
                BUTTON_SIGNAL.signal(true);
            },
            Duration::from_millis(100)
        )]
    );
    log::info!("Button'lar baslatildi");

    loop {
        Timer::after(Duration::from_millis(1000)).await;
        let val = BUTTON_SIGNAL.wait().await;
        log::info!("Button yakalandi. val: {}", val);
    }
}
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
//////////////////////////////////////////////////////////////////////////////////
