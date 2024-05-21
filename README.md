# esp32 scopeclock

Analog scopeclock with Z blanking and NTP client running on esp32 written in Rust

![Animation of the scopeclock on a Fluke oscilloscope](doc/render.gif)

## Project goal

* Learning to code in embedded Rust on the ESP32
* Avoiding esp-idf Rust wrapper if possible
* Having a scope clock ^^

## Features

* Shows an analog clock face with 3 clock hands and AM/PM display
* XY signal generated using the internal 2 channel 8 bit DAC
* DAC is operated in DMA mode, allowing the CPU to continue with different tasks
* Z Blanking is implemented using a NMI routine written in assembly (see below in the FAQ)
* Uses embassy as RTOS
* NTP client for time keeping
* MQTT client included for testing, but not used for the clock right now.

## How to build the software

### Prerequisites

If not yet installed, you need to [have a Rust environment](https://www.rust-lang.org/tools/install).
Afterwards we need to take care of a certain weirdness of developing Rust for the esp32.
Usually the compiler is multi platform, but the Tensilica Xtensa is excluded from that.
Smart people have taken care of the previously tedious process of installing the custom toolchain by doing this:

    cargo install espup
    espup install

Afterwards just use

    . ~/export-esp.sh

to activate the ESP32 Rust environment. This needs to be done before usage as the linker process will fail.

## espflash

Might be required for flashing

    cargo install espflash

### Building

No configuration system available right now. SSID and password
need to be provided by environment variables:

    export SSID="ssid"
    export PASSWORD="no_idea"

To build

    cargo build --release

### Flashing

To flash and monitor

    cargo run --release

## Building the hardware

Right now the circuit is very primitive and a ESP32 board is enough. I suggest adding some resistors and capacitors for filtering though.
Here an example build:

![Photo of ESP32 on breadboard with some resistors](doc/breadboard.png)

## TODOs

* Remove `static mut` which is currently required for the ownership in IRQ handlers
    * For some reason this seems to be fixed with the [cortex-m-rt](https://docs.rs/cortex-m-rt-macros/0.1.5/cortex_m_rt_macros/attr.interrupt.html) crate when ARM cpus are used. But the esp32 has nothing like it. Are the IRQ handlers re-entrant?
* Provide the enhancements as a PR to esp-hal
* Add a panic handler which protects the tube from damage
* Replace the MQTT library which might not be very stable
* Fix the build of the Font generator subproject

## FAQ

### Why write something in assembly for this project?

The Tensilica Xtensa is a rather weird CPU core. It makes use of a moving window register file to have the
stack partially embedded inside the core itself, avoiding to access the memory during function calls.
This register file has a limited size, causing an interrupt in case it overflows as the CPU now needs to rescue the contents
to the real stack. This leads to unpredictable latencies not only during normal execution but also during interrupt service routines.
The DMA of the ESP32 has as a limited feature set and I wanted to be able to activate a Z Blank using GPIOs between drawing lines. Doing
that inside a normal ISR caused weird flickering as the latency of the ISR fluctuated between 2µs and 60µs. Having WiFi active worsened the issue.

My solution was the usage of [High Priority Interrupts](https://docs.espressif.com/projects/esp-idf/en/stable/esp32/api-guides/hlinterrupts.html).

These ISRs are nested and have to be written in assembly as only a part of the instruction set must be used.
The moving window instructions are forbidden. The documentation was pretty bad but in the end, it worked and is stable.

### error: linker \`xtensa-esp32-elf-gcc\` not found

Please read the prerequisites again...
But maybe you just forgot `. ~/export-esp.sh`