use alloc::vec::Vec;

use chrono::{DateTime, Timelike};
use chrono::{Datelike, TimeZone};
use libm::ceilf;


use crate::picture::{Picture, StaticPartMeta};
use crate::{font, ntptime};
use bresenham::Point;

use esp_backtrace as _;

use sntpc::NtpTimestampGenerator;

/// either 1 or 2
pub const GLOBAL_SCALE: isize = 2;

/// Converts radial coordinate int cartesian
/// Provide phi in range of 0 to 2*PI
/// Returns coordinates with Y+ going up assuming a vector scope coordinate system
/// Phi of 0 provides North, Phi of PI/2 provides East and so on.
/// So this function is clock wise
pub fn radial_to_cartesian(phi: f32, radius: f32) -> Point {
    // TODO What is better? ceilf or roundf?
    let x = ceilf(libm::sinf(phi) * radius) as isize + 0x82 * GLOBAL_SCALE;
    let y = ceilf(libm::cosf(phi) * radius) as isize + 0x82 * GLOBAL_SCALE;
    (x, y)
}

pub fn radial_to_cartesian_uncentered(phi: f32) -> (f32, f32) {
    // TODO What is better? ceilf or roundf?
    let x = libm::sinf(phi);
    let y = libm::cosf(phi);
    (x, y)
}

fn draw_static_clock_face(pic: &mut Picture) {
    // Big circle as a round bezel
    pic.add_circle(
        (0x82 * GLOBAL_SCALE, 0x82 * GLOBAL_SCALE),
        (0x7d * GLOBAL_SCALE) as f32,
        40,
    );

    // Circle in the center because it is cute
    pic.add_circle(
        (0x82 * GLOBAL_SCALE, 0x82 * GLOBAL_SCALE),
        (0x0a * GLOBAL_SCALE) as f32,
        10,
    );

    // Draw the clock face with
    // - dots at the minute marks
    // - circles at the hour marks
    // - numbers at the hour marks
    for i in 0..60 {
        let phi = (i as f32 / 60.0) * core::f32::consts::PI * 2.0;
        let outer = radial_to_cartesian(phi, (0x58 * GLOBAL_SCALE) as f32);
        let outer_number = radial_to_cartesian(phi, (0x6c * GLOBAL_SCALE) as f32);
        //pic.add_line(inner, outer);

        if i % 5 == 0 {
            pic.add_double_circle(outer, 2.5 * GLOBAL_SCALE as f32, 8);

            let number = if i == 0 { 12 } else { i / 5 };
            let drawing = &font::FONT[number];

            for line in drawing.lines {
                let scaler = 9_f32 * GLOBAL_SCALE as f32;

                let line: Vec<Point> = line
                    .iter()
                    .map(|p| {
                        let x = (p.0 * scaler) as isize + outer_number.0;
                        // Mirror Y as scopes show Y+ to up
                        // but on PCs Y usually goes down
                        let y = (-p.1 * scaler) as isize + outer_number.1;
                        (x, y)
                    })
                    .collect();
                pic.add_open_polygon(&line);
            }
        } else {
            pic.add_dot2(outer, 14);
        }
    }
}

fn draw_dynamic_parts(pic: &mut Picture) {
    let local_time = critical_section::with(|cs| ntptime::PUBLIC_TIME.borrow(cs).get());

    // Draw the hands if we have the time to present
    if let Some(mut local_time) = local_time {
        local_time.init();
        let secs = local_time.timestamp_sec();
        let nano = local_time.timestamp_subsec_micros() * 1000;

        // Create Unix timestamp
        let utc = DateTime::from_timestamp(secs as i64, nano).unwrap();
        // Create a normal DateTime from the NaiveDateTime
        // to get the local time.
        // TODO At the moment limited to german time
        let local_time = chrono_tz::Europe::Berlin.from_utc_datetime(&utc.naive_utc());

        // Seconds - stalling
        /*
        let phi = (local_time.second() as f32 / 60.0) * core::f32::consts::PI * 2.0;
        let inner = radial_to_cartesian(phi, (0x10 * GLOBAL_SCALE) as f32);
        let outer = radial_to_cartesian(phi, (0x54 * GLOBAL_SCALE) as f32);
        pic.add_line(inner, outer);
        */

        // Seconds - smooth sweep
        let second_fraction = local_time.timestamp_subsec_millis() as f32 / 1000.0;
        let circle_fraction = (local_time.second() as f32 + second_fraction) / 60.0;
        let seconds_phi = circle_fraction * core::f32::consts::PI * 2.0;
        let inner = radial_to_cartesian(seconds_phi, (0x10 * GLOBAL_SCALE - 1) as f32);
        let outer = radial_to_cartesian(seconds_phi, (0x60 * GLOBAL_SCALE - 1) as f32);
        pic.add_line(inner, outer);

        // Minutes - stalling
        /*
        let mut poly: Vec<Point> = Vec::with_capacity(4);
        let phi = (local_time.minute() as f32 / 60.0) * core::f32::consts::PI * 2.0;
        poly.push(radial_to_cartesian(phi, (0x10 * GLOBAL_SCALE) as f32));
        poly.push(radial_to_cartesian(phi - 0.1, (0x28 * GLOBAL_SCALE) as f32));
        poly.push(radial_to_cartesian(phi, (0x50 * GLOBAL_SCALE) as f32));
        poly.push(radial_to_cartesian(phi + 0.1, (0x28 * GLOBAL_SCALE) as f32));
        pic.add_closed_polygon(&poly);
        */

        // Minutes - smooth sweep
        let mut poly: Vec<Point> = Vec::with_capacity(4);
        let minute_fraction = local_time.second() as f32 / 60.0;
        let minutes_phi =
            ((local_time.minute() as f32 + minute_fraction) / 60.0) * core::f32::consts::PI * 2.0;
        poly.push(radial_to_cartesian(
            minutes_phi,
            (0x10 * GLOBAL_SCALE) as f32,
        ));
        poly.push(radial_to_cartesian(
            minutes_phi - 0.1,
            (0x28 * GLOBAL_SCALE) as f32,
        ));
        poly.push(radial_to_cartesian(
            minutes_phi,
            (0x50 * GLOBAL_SCALE) as f32,
        ));
        poly.push(radial_to_cartesian(
            minutes_phi + 0.1,
            (0x28 * GLOBAL_SCALE) as f32,
        ));
        pic.add_closed_polygon(&poly);

        // Hours - stalling
        /*
        let mut hour_poly: Vec<Point> = Vec::with_capacity(4);

        let phi = (local_time.hour12().1 as f32 / 12.0) * core::f32::consts::PI * 2.0;
        hour_poly.push(radial_to_cartesian(phi, (0x10 * GLOBAL_SCALE) as f32));
        hour_poly.push(radial_to_cartesian(phi - 0.3, (0x20 * GLOBAL_SCALE) as f32));
        hour_poly.push(radial_to_cartesian(phi, (0x40 * GLOBAL_SCALE) as f32));
        hour_poly.push(radial_to_cartesian(phi + 0.3, (0x20 * GLOBAL_SCALE) as f32));
        pic.add_closed_polygon(&hour_poly);
        */
        // Hours - smooth sweep
        let mut hour_poly: Vec<Point> = Vec::with_capacity(4);

        let hours = local_time.hour12();
        // handle special case of 12 being 0 at the top
        // subtract 1 to move range from 1..12 to 0..11
        let hours12 = hours.1 as f32;
        let minutes_hour_fraction = local_time.minute() as f32 / 60.0;
        let circle_fraction = (hours12 + minutes_hour_fraction) / 12.0;
        //println!("{}", circle_fraction);
        let hours_phi = circle_fraction * core::f32::consts::PI * 2.0;
        hour_poly.push(radial_to_cartesian(hours_phi, (0x10 * GLOBAL_SCALE) as f32));
        hour_poly.push(radial_to_cartesian(
            hours_phi - 0.3,
            (0x20 * GLOBAL_SCALE) as f32,
        ));
        hour_poly.push(radial_to_cartesian(hours_phi, (0x40 * GLOBAL_SCALE) as f32));
        hour_poly.push(radial_to_cartesian(
            hours_phi + 0.3,
            (0x20 * GLOBAL_SCALE) as f32,
        ));
        pic.add_closed_polygon(&hour_poly);

        // AM and PM

        let scaler = 7.5_f32 * GLOBAL_SCALE as f32;

        // Use some vector math to put the AM/PM at exactly the opposite center
        // of the hours and minute hand..
        // Might look weird. Let's see
        let (translate_x1, translate_y1) = radial_to_cartesian_uncentered(hours_phi);
        let (translate_x2, translate_y2) = radial_to_cartesian_uncentered(minutes_phi);
        let translate_x = -translate_x1 - translate_x2;
        let translate_y = -translate_y1 - translate_y2;

        let length = libm::sqrtf(translate_x * translate_x + translate_y * translate_y);
        let translate_x =
            (45_f32 * GLOBAL_SCALE as f32 * translate_x / length) as isize + 0x82 * GLOBAL_SCALE;
        let translate_y =
            (45_f32 * GLOBAL_SCALE as f32 * translate_y / length) as isize + 0x82 * GLOBAL_SCALE;

        let show_date = (local_time.second() % 10) >= 5;
        // let show_date = (local_time.second() % 2) >= 1;
        if show_date {
            let day = local_time.day();
            let month = local_time.month();
            //let month = 12;

            let day_digit0 = (day / 10) as usize;
            let day_digit1 = (day % 10) as usize;
            // if day_digit0 > 0
            {
                let pictogram = &font::FONT[day_digit0];
                pic.draw_font(pictogram, scaler, translate_x - 15 - 25, translate_y + 15);
            }
            let pictogram = &font::FONT[day_digit1];
            pic.draw_font(pictogram, scaler, translate_x + 15 - 25, translate_y + 15);
            let pictogram = &font::FONT[month as usize];
            pic.draw_font(
                pictogram,
                scaler,
                translate_x + 15 + 30 - 20,
                translate_y - 30 + 15,
            );
        } else {
            let is_pm = hours.0;
            let index = if is_pm { 14 } else { 13 };
            let pictogram = &font::FONT[index];
            pic.draw_font(pictogram, scaler, translate_x, translate_y);
        }
    }

    // bring the beam to a position to rest until the next picture
    pic.add_raw_point(0, 0);
}

pub fn prepare_static_part(tx_buffer: &mut [u8]) -> StaticPartMeta {
    let mut pic = Picture::new(tx_buffer);
    draw_static_clock_face(&mut pic);
    return StaticPartMeta {
        out_index: pic.out_index,
        parts: pic.parts,
    };
}

pub fn draw_dynamic_part<'a, 'b>(
    tx_buffer: &'a mut [u8],
    static_part: &'b StaticPartMeta,
) -> Picture<'a> {
    let mut pic = Picture::new(tx_buffer);
    pic.out_index = static_part.out_index;
    pic.parts = static_part.parts.clone();
    draw_dynamic_parts(&mut pic);
    pic
}
