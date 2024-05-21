#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(asm_experimental_arch, naked_functions)]

extern crate alloc;

use core::mem::MaybeUninit;

use embassy_executor::Spawner;

use embassy_net::{Config, Stack, StackResources};
#[path = "util.rs"]
mod examples_util;
use esp_wifi_sys::include::esp_wifi_set_max_tx_power;
use examples_util::hal;

use embassy_time::{Duration, Timer};
use esp_backtrace as _;

use esp_println::println;
use esp_wifi::wifi::{ClientConfiguration, Configuration};
use esp_wifi::wifi::{WifiController, WifiDevice, WifiEvent, WifiStaDevice, WifiState};
use esp_wifi::{initialize, EspWifiInitFor};
use hal::analog::dac::{set_dma_mode, DAC1, DAC2};
use hal::clock::ClockControl;
use hal::dma::Dma;
use hal::gpio::{DriveStrength, IO};

use hal::rng::Rng;

use hal::{embassy, peripherals::Peripherals, prelude::*, timer::TimerGroup};

use static_cell::make_static;

mod analog_clock_face;
mod font;
mod httptest;
mod mqtt;
mod ntptime;
mod picture;
mod scopeclock;

use crate::httptest::http_stuff;
use crate::mqtt::mqtt_stuff;
use crate::ntptime::time_stuff;
use crate::scopeclock::{scopeclock_init, scopeclock_task};

const SSID: &str = env!("SSID");
const PASSWORD: &str = env!("PASSWORD");

#[global_allocator]
static ALLOCATOR: esp_alloc::EspHeap = esp_alloc::EspHeap::empty();

fn init_heap() {
    const HEAP_SIZE: usize = 8 * 1024;
    static mut HEAP: MaybeUninit<[u8; HEAP_SIZE]> = MaybeUninit::uninit();

    unsafe {
        ALLOCATOR.init(HEAP.as_mut_ptr() as *mut u8, HEAP_SIZE);
    }
}

#[main]
async fn main(spawner: Spawner) -> ! {
    #[cfg(feature = "log")]
    esp_println::logger::init_logger(log::LevelFilter::Info);

    let peripherals = Peripherals::take();

    let system = peripherals.SYSTEM.split();

    let clocks = ClockControl::max(system.clock_control).freeze();
    let delay = hal::delay::Delay::new(&clocks);
    init_heap();

    #[cfg(target_arch = "xtensa")]
    let timer = hal::timer::TimerGroup::new(peripherals.TIMG1, &clocks).timer0;
    #[cfg(target_arch = "riscv32")]
    let timer = hal::systimer::SystemTimer::new(peripherals.SYSTIMER).alarm0;
    let init = initialize(
        EspWifiInitFor::Wifi,
        timer,
        Rng::new(peripherals.RNG),
        system.radio_clock_control,
        &clocks,
    )
    .unwrap();

    let wifi = peripherals.WIFI;
    let (wifi_interface, controller) =
        esp_wifi::wifi::new_with_mode(&init, wifi, WifiStaDevice).unwrap();

    let timer_group0 = TimerGroup::new(peripherals.TIMG0, &clocks);
    embassy::init(&clocks, timer_group0);

    let config = Config::dhcpv4(Default::default());

    let seed = 1234; // very random, very secure seed

    // Init network stack
    let stack = &*make_static!(Stack::new(
        wifi_interface,
        config,
        make_static!(StackResources::<5>::new()),
        seed
    ));

    let io = IO::new(peripherals.GPIO, peripherals.IO_MUX);
    let dac1_pin: hal::gpio::GpioPin<hal::gpio::Analog, 25> = io.pins.gpio25.into_analog();
    let dac2_pin = io.pins.gpio26.into_analog();
    let mut z_blank = io.pins.gpio32.into_push_pull_output();

    z_blank.set_output_high(true);
    z_blank.set_drive_strength(DriveStrength::I5mA);
    z_blank.enable_output(true);

    let mut _dac1 = DAC1::new(peripherals.DAC1, dac1_pin);
    let mut _dac2 = DAC2::new(peripherals.DAC2, dac2_pin);
    set_dma_mode(true);

    let dma = Dma::new(peripherals.DMA);
    let dma_channel = dma.i2s0channel;
    let i2s = peripherals.I2S0;

    let static_part_meta = scopeclock_init(i2s, &clocks, dma_channel, z_blank, delay);

    spawner.spawn(connection(controller)).ok();
    spawner.spawn(net_task(stack)).ok();
    spawner.spawn(scopeclock_task(static_part_meta)).ok();

    loop {
        if stack.is_link_up() {
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    println!("Waiting to get IP address...");
    loop {
        if let Some(config) = stack.config_v4() {
            println!("Got IP: {}", config.address);
            break;
        }
        Timer::after(Duration::from_millis(500)).await;
    }

    //spawner.spawn(http_stuff(stack)).ok();
    spawner.spawn(time_stuff(stack)).ok();
    //spawner.spawn(mqtt_stuff(stack)).ok();

    // endless loop
    loop {
        Timer::after(Duration::from_millis(5000)).await;
        //println!("Still alive...");
    }
}

#[embassy_executor::task]
async fn connection(mut controller: WifiController<'static>) {
    println!("start connection task");
    println!("Device capabilities: {:?}", controller.get_capabilities());
    loop {
        if let WifiState::StaConnected = esp_wifi::wifi::get_wifi_state() {
            // wait until we're no longer connected
            controller.wait_for_event(WifiEvent::StaDisconnected).await;
            Timer::after(Duration::from_millis(5000)).await
        }
        if !matches!(controller.is_started(), Ok(true)) {
            let client_config = Configuration::Client(ClientConfiguration {
                ssid: SSID.try_into().unwrap(),
                password: PASSWORD.try_into().unwrap(),
                ..Default::default()
            });
            controller.set_configuration(&client_config).unwrap();
            println!("Starting wifi");
            controller.start().await.unwrap();
            println!("Wifi started!");
            let retval = unsafe {esp_wifi_set_max_tx_power(8)};
            println!("tx power {}",retval);
        }
        println!("About to connect...");

        match controller.connect().await {
            Ok(_) => println!("Wifi connected!"),
            Err(e) => {
                println!("Failed to connect to wifi: {e:?}");
                Timer::after(Duration::from_millis(5000)).await
            }
        }
    }
}

#[embassy_executor::task]
async fn net_task(stack: &'static Stack<WifiDevice<'static, WifiStaDevice>>) {
    stack.run().await
}
