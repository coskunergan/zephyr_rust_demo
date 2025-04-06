
use embassy_time::{Duration, Timer};

use zephyr::{
    device::gpio::{GpioPin, GpioToken},
    raw::{GPIO_OUTPUT_ACTIVE},
    sync::{Arc, Mutex},
};

use log::info;

pub struct Led {
    pin: GpioPin,
    delay: Duration,
    token: Arc<Mutex<zephyr::device::gpio::GpioToken>>, // Thread-safe paylaşım için
}

impl Led {    
    pub fn new(pin: GpioPin, delay: Duration, token: Arc<Mutex<zephyr::device::gpio::GpioToken>>) -> Self {
        Self { pin, delay, token }
    }
    
    pub async fn blinky(&mut self) {
        
        let mut token_lock = self.token.lock().unwrap();            
             
        unsafe {
                self.pin.configure(&mut token_lock, GPIO_OUTPUT_ACTIVE);
            }                    
            
        loop{
            unsafe { self.pin.toggle_pin(&mut token_lock); }                                                   
                 
            Timer::after(self.delay).await;
        }        
    }
}

#[embassy_executor::task]
pub async fn led_task(mut led: Led) {
    led.blinky().await;
}

// Birden çok LED'i deklare eden ve çalıştıran makro
#[macro_export]
macro_rules! declare_leds {
    ($spawner:expr, $token:expr, [ $( ($pin:expr, $delay:expr) ),* ]) => {
        {
            $(
                let pin = $pin;
                let delay = $delay;                
                let mut led = $crate::led::Led::new(pin, delay, $token.clone());                
                $spawner.spawn($crate::led::led_task(led)).unwrap();
            )*
        }
    };
}