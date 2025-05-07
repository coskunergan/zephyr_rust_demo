// Copyright (c) 2025
// SPDX-License-Identifier: Apache-2.0
// Coskun ERGAN <coskunergan@gmail.com>

use embassy_time::{Duration, Timer};

use alloc::boxed::Box;
use zephyr::{
    raw::GPIO_INPUT,
    sync::{Arc, Mutex},
};

use super::{GpioPin, GpioToken};
use log::warn;

pub struct Button {
    token: Arc<Mutex<GpioToken>>,
    pin: GpioPin,
    callback: Box<dyn Fn() + Send + Sync + 'static>,
    debounce: Duration,
}

impl Button {
    pub fn new(
        token: Arc<Mutex<GpioToken>>,
        pin: GpioPin,
        callback: Box<dyn Fn() + Send + Sync + 'static>,
        debounce: Duration,
    ) -> Self {
        Self {
            token,
            pin,
            callback,
            debounce,
        }
    }
    #[allow(dead_code)]
    pub fn set_callback(&mut self, cb: Box<dyn Fn() + Send + Sync + 'static>) {
        self.callback = cb;
    }
    #[allow(dead_code)]
    pub fn trigger_callback(&self) {
        (self.callback)();
    }

    pub async fn work(&mut self) {
        let mut token_lock = self.token.lock().unwrap();

        if !self.pin.is_ready() {
            warn!("Button pin is not ready");
            loop {}
        }

        unsafe {
            self.pin.configure(&mut token_lock, GPIO_INPUT);
        }

        loop {
            unsafe { self.pin.wait_for_high(&mut token_lock).await };

            (self.callback)();

            Timer::after(self.debounce).await;

            unsafe { self.pin.wait_for_low(&mut token_lock).await };

            Timer::after(self.debounce).await;
        }
    }
}

#[macro_export]
macro_rules! declare_buttons {
    ($spawner:expr, $token:expr, [ $( ($pin:expr, $closure:expr, $debounce:expr) ),* ]) => {
        {
            const BUTTON_COUNT: usize = 0 $( + { let _ = ($debounce); 1 } )*;
            log::info!("Declared button count: {}", BUTTON_COUNT);

            #[embassy_executor::task(pool_size = BUTTON_COUNT)]
            async fn button_task(mut button: crate::button::Button) {
                button.work().await;
            }

            $(
                use alloc::boxed::Box;
                let pin = $pin;
                let debounce = $debounce;
                let button = $crate::button::Button::new($token.clone(), pin, Box::new($closure), debounce);
                match $spawner.spawn(button_task(button)) {
                    Ok(_) => log::info!("Button task started."),
                    Err(e) => log::error!("Button task failure: {:?}", e),
                }
            )*
        }
    };
}
