use core::cell::RefCell;
use core::sync::atomic::AtomicU32;

use critical_section::Mutex;

use hal::delay::Delay;
use hal::gpio::{GpioPin, Output, PushPull};
use hal::system::SoftwareInterrupt;

#[path = "util.rs"]
mod examples_util;

use crate::analog_clock_face::{draw_dynamic_part, prepare_static_part};
use crate::picture::{Picture, StaticPartMeta};

use embassy_time::{Duration, Instant, Timer};
use esp_backtrace as _;
use esp_hal::gpio::OutputPin;
use esp_hal::i2s::{DataFormat, I2s, I2sTx, I2sWriteDma, I2sWriteDmaTransfer, Standard};
use esp_println::println;
use examples_util::hal;
use hal::clock::Clocks;
use hal::dma::{DmaPriority, I2s0DmaChannel, I2s0DmaChannelCreator};
use hal::interrupt::{CpuInterrupt, Priority};
use hal::peripherals::{Interrupt, Peripherals, I2S0};
use hal::prelude::*;
use hal::{dma_buffers, interrupt, Blocking};

use static_cell::make_static;

struct DmaData {
    canvas: Option<&'static mut [u8]>,
    next_display: Option<Picture<'static>>,
    z_blank: GpioPin<Output<PushPull>, 32>,
    delay: Delay,
}
pub static WAIT_BEFORE_BEAM_OFF: AtomicU32 = AtomicU32::new(10);
pub static WAIT_AFTER_BEAM_ON: AtomicU32 = AtomicU32::new(0);

static DMA_DATA: Mutex<RefCell<Option<DmaData>>> = Mutex::new(RefCell::new(None));

fn draw_picture<'a, 'b>(
    tx_buffer: &'a mut [u8],
    static_part_meta: &'b StaticPartMeta,
) -> Picture<'a> {
    draw_dynamic_part(tx_buffer, static_part_meta)
}

pub fn scopeclock_init(
    i2s: I2S0,
    clocks: &Clocks,
    dma_channel: I2s0DmaChannelCreator,
    z_blank: GpioPin<Output<PushPull>, 32>,
    delay: Delay,
) -> StaticPartMeta {
    let (tx_buffer1, tx_descriptors, _, rx_descriptors) = dma_buffers!(50000, 0);
    let (tx_buffer2, _, _, _) = dma_buffers!(50000, 0);

    // For reasons I don't understand, dma_buffers!(...) doesn't provide descriptors with static lifetime.
    // We need to correct that here as the descriptors need to outlive I2s.
    let tx_descriptors = make_static!(tx_descriptors);
    let rx_descriptors = make_static!(rx_descriptors);

    println!("{:?} descriptors", tx_descriptors.len());

    let i2s = I2s::new(
        i2s,
        Standard::DAC,
        DataFormat::Data16Channel16,
        (44100 * 3).Hz(),
        dma_channel.configure(
            false,
            tx_descriptors,
            rx_descriptors,
            DmaPriority::Priority0,
        ),
        clocks,
    );
    let start = Instant::now();

    let _static_part_meta = prepare_static_part(tx_buffer1);
    let static_part_meta = prepare_static_part(tx_buffer2);

    let drawing1 = draw_picture(tx_buffer1, &static_part_meta);
    println!("Drawing took {:?}ms", start.elapsed().as_millis());
    let drawing2 = draw_picture(tx_buffer2, &static_part_meta);

    println!("{} bytes", drawing1.out_index);

    let dma_data = DmaData {
        canvas: None,
        next_display: Some(drawing1),
        z_blank,
        delay,
    };

    critical_section::with(|cs| {
        DMA_DATA.borrow_ref_mut(cs).replace(dma_data);
    });

    unsafe {
        TX.replace(i2s.i2s_tx.build());
        CURRENT_DISPLAY.replace(drawing2);
        update_frame();
    };

    interrupt::enable_direct(Interrupt::I2S0, CpuInterrupt::Interrupt14NmiPriority7).unwrap();
    interrupt::enable(Interrupt::FROM_CPU_INTR3, Priority::Priority3).unwrap();

    //interrupt::enable(Interrupt::I2S0, Priority::Priority3).unwrap();
    static_part_meta
}

#[embassy_executor::task]
pub async fn scopeclock_task(static_part_meta: StaticPartMeta) {
    loop {
        // Take the canvas if it exists
        let canvas = critical_section::with(|cs| {
            let mut dma_data = DMA_DATA.borrow_ref_mut(cs);
            let dma_data = dma_data.as_mut().unwrap();

            dma_data.canvas.take()
        });

        // If there was a canvas we took, draw on it
        if let Some(canvas) = canvas {
            //let start = Instant::now();
            let drawing = draw_picture(canvas, &static_part_meta);
            //println!("Drawing took {:?}ms", start.elapsed().as_millis());
            //println!("{} bytes", drawing.out_index);

            critical_section::with(|cs| {
                let mut dma_data = DMA_DATA.borrow_ref_mut(cs);
                let dma_data = dma_data.as_mut().unwrap();

                // Place the drawing into the next_display
                dma_data.next_display.replace(drawing);
            });
        }

        Timer::after(Duration::from_millis(5)).await;
    }
}

// Must be shared mutable to allow usage with the DMA for now. write_dma of I2sTx demands it
static mut CURRENT_DISPLAY: Option<Picture<'static>> = None;

// Write_dma call requires I2sTx being borrowed for static
static mut TX: Option<I2sTx<'static, I2S0, I2s0DmaChannel, Blocking>> = None;

#[ram]
unsafe fn select_next_picture(dma_data: &mut DmaData) {
    if let Some(next) = dma_data.next_display.take() {
        // So we have a next picture to display?
        // Move canvas <- current to have another free canvas to draw on
        // current <- next
        // next is left empty in that case
        let current = CURRENT_DISPLAY.take().unwrap();
        dma_data.canvas = Some(current.tx_buffer);
        CURRENT_DISPLAY.replace(next);
    } else {
        //println!("Missed frame");
    }
}

#[ram]
// Function is not reentrant!
unsafe fn update_frame() {
    static mut TRANSFER: Option<
        I2sWriteDmaTransfer<'static, 'static, I2S0, I2s0DmaChannel, Blocking>,
    > = None;

    //let transfer_line_for_line = (embassy_time::Instant::now().as_secs() % 10) >= 5;
    let transfer_line_for_line = true;
    let stolen_peripheral = Peripherals::steal(); // TODO find a way around this
    let stolen_i2s0 = stolen_peripheral.I2S0;

    critical_section::with(|cs| {
        let mut dma_data = DMA_DATA.borrow_ref_mut(cs);
        let dma_data = dma_data.as_mut().unwrap();

        if let Some(mut t) = TRANSFER.take() {
            t.clear_int();
            t.wait().unwrap();
        }

        // look for next picture to show
        let tx_slice = if transfer_line_for_line {
            let current_display = CURRENT_DISPLAY.as_mut().unwrap();

            let indizes =
                if let Some(indizes) = current_display.parts.get(current_display.current_part) {
                    //current_display.parts.remove(0)
                    current_display.current_part += 1;
                    *indizes
                } else {
                    select_next_picture(dma_data);

                    // CURRENT_DISPLAY was swapped. Let's grab it again
                    let current_display = CURRENT_DISPLAY.as_mut().unwrap();
                    current_display.current_part = 1;
                    current_display.parts[0]
                };
            let to_draw = CURRENT_DISPLAY.as_mut().unwrap();
            let tx_slice = &to_draw.tx_buffer[indizes.0..indizes.1];
            tx_slice
        } else {
            select_next_picture(dma_data);

            let to_draw = CURRENT_DISPLAY.as_ref().unwrap();
            let tx_slice = &to_draw.tx_buffer[0..to_draw.out_index];
            tx_slice
        };

        // Make a small transfer to establish the first required sample for the next line
        // This gives us the possibility to wait some time before disabling blank
        let tx: &mut I2sTx<'_, I2S0, I2s0DmaChannel, Blocking> = TX.as_mut().unwrap();
        let tx_startslice = &tx_slice[0..4];
        let transfer = tx.write_dma(&tx_startslice).unwrap();
        // delay to give the DMA some time to activate
        dma_data.delay.delay_nanos(10);
        transfer.wait().unwrap();

        //dma_data.delay.delay_micros(10);

        // before leaving function, put tx_slice into something with static lifetime
        static mut CURRENT_TX_SLICE: Option<&[u8]> = None;
        let tx_slice = {
            CURRENT_TX_SLICE.replace(tx_slice);
            CURRENT_TX_SLICE.as_ref().unwrap()
        };

        let transfer = tx.write_dma(tx_slice).unwrap();
        stolen_i2s0
            .int_clr()
            .write(|f| f.tx_rempty().clear_bit_by_one());

        TRANSFER = Some(transfer);

        // enable the beam after a short pause
        dma_data.z_blank.set_output_high(false);
        // delay to give the DMA some time to activate
        dma_data.delay.delay_nanos(10);

        stolen_i2s0
            .int_clr()
            .write(|f| f.tx_rempty().clear_bit_by_one());
        stolen_i2s0.int_ena().write(|f| f.tx_rempty().set_bit());
    });
}

use core::arch::asm;
core::arch::global_asm!(include_str!("high_level.S"));

#[naked]
#[no_mangle]
#[link_section = ".iram1"]
unsafe extern "C" fn __naked_level_7_interrupt() {
    asm!("HANDLE_INTERRUPT_LEVEL2 7", options(noreturn));
}

#[ram]
#[interrupt]
fn FROM_CPU_INTR3() {
    // Clear the software interrupt
    let stolen_peripheral = unsafe { Peripherals::steal() };
    let system = stolen_peripheral.SYSTEM.split();
    let mut sw_int = system.software_interrupt_control;
    sw_int.reset(SoftwareInterrupt::SoftwareInterrupt3);

    unsafe { update_frame() };
}
