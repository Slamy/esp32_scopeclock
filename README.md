# esp32 scopeclock

Analog scopeclock with Z blanking and NTP client running on esp32 written in Rust

## Project goal

* Learning to code in embedded Rust on the ESP32
* Avoiding esp-idf if possible
* Having a scope clock

## Features

* XY signal generated using the internal 8 bit DAC
* DAC is operated in DMA mode, allowing to continue the CPU with different tasks
* Z Blanking is implemented using NMI routine written in assembly (see below)
* Uses embassy as RTOS
* NTP client for time keeping



