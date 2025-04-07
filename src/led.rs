use embassy_time::{Duration, Instant, Timer};

use zephyr::{
    raw::GPIO_OUTPUT_ACTIVE,
    sync::{Arc, Mutex},
};

use super::{GpioPin, GpioToken};

use log::info;

pub struct Led {
    pin: GpioPin,
    delay: Duration,
    token: Arc<Mutex<GpioToken>>,
}

impl Led {
    pub fn new(pin: GpioPin, delay: Duration, token: Arc<Mutex<GpioToken>>) -> Self {
        Self { pin, delay, token }
    }

    pub async fn blinky(&mut self) {
        let mut token_lock = self.token.lock().unwrap();

        unsafe {
            self.pin.configure(&mut token_lock, GPIO_OUTPUT_ACTIVE);
        }

        loop {
            unsafe {
                self.pin.toggle_pin(&mut token_lock);
            }

            Timer::after(self.delay).await;
        }
    }
}

#[macro_export]
macro_rules! declare_leds {
    ($spawner:expr, $token:expr, [ $( ($pin:expr, $delay:expr) ),* ]) => {
        {
            const LED_COUNT: usize = 0 $( + { let _ = ($delay); 1 } )*;
            log::info!("Deklare edilen LED sayisi: {}", LED_COUNT);

            #[embassy_executor::task(pool_size = LED_COUNT)]
            async fn led_task(mut led: crate::led::Led) {
                led.blinky().await;
            }

            $(
                let pin = $pin;
                let delay = $delay;
                let mut led = $crate::led::Led::new(pin, delay, $token.clone());
                match $spawner.spawn(led_task(led)) {
                    Ok(_) => log::info!("LED gorevi baslatildi."),
                    Err(e) => log::error!("LED gorevi baslatilamadi: {:?}", e),
                }
            )*
        }
    };
}
