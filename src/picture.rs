use alloc::vec::Vec;

#[path = "util.rs"]
mod examples_util;
use crate::{analog_clock_face::GLOBAL_SCALE, font::Drawing};

use bresenham::Bresenham;

use esp_backtrace as _;

use libm::{ceilf, roundf};

type Point = (isize, isize);

fn closed_polygon_to_lines<F>(points: &[Point], mut f: F)
where
    F: FnMut(Point, Point),
{
    let first = points.iter();
    let mut second = points.iter();
    second.next();
    for p1 in first {
        let p2 = second.next().unwrap_or(points.first().unwrap());
        f(*p1, *p2);
    }
}

fn open_polygon_to_lines<F>(points: &[Point], mut f: F)
where
    F: FnMut(Point, Point),
{
    let first = points.iter();
    let mut second = points.iter();
    second.next();
    for p1 in first {
        if let Some(p2) = second.next() {
            f(*p1, *p2);
        }
    }
}

pub struct Picture<'a> {
    pub tx_buffer: &'a mut [u8],
    pub out_index: usize,
    pub parts: Vec<(usize, usize)>,
    pub current_part: usize,
    //iter: alloc::slice::Iter<'static, (usize, usize)>,
}

impl<'a> Picture<'a> {
    pub fn new(tx_buffer: &'a mut [u8]) -> Picture<'a> {
        Self {
            tx_buffer,
            out_index: 0,
            parts: Vec::new(),
            current_part: 0,
            // iter: alloc::slice::Iter::default(),
        }
    }

    pub fn add_point(&mut self, x: u16, y: u16) {
        if GLOBAL_SCALE == 2 {
            self.add_raw_point((x >> 1) as u8, (y >> 1) as u8);

            let second_x = if (x & 1) == 1 {
                ((x + 1) >> 1) as u8
            } else {
                (x >> 1) as u8
            };

            let second_y = if (y & 1) == 1 {
                ((y + 1) >> 1) as u8
            } else {
                (y >> 1) as u8
            };

            self.add_raw_point(second_x, second_y);
        } else {
            self.add_raw_point(x as u8, y as u8);
        }
    }

    pub fn add_raw_point(&mut self, x: u8, y: u8) {
        self.tx_buffer[self.out_index + 0] = 0;
        self.tx_buffer[self.out_index + 1] = x as u8;
        self.tx_buffer[self.out_index + 2] = 0;
        self.tx_buffer[self.out_index + 3] = y as u8;
        self.out_index += 4;
    }

    pub fn add_dot(&mut self, x: u16, y: u16, exposure: usize) {
        let start_index = self.out_index;
        for _ in 0..exposure {
            self.add_point(x, y)
        }
        self.parts.push((start_index, self.out_index));
    }

    pub fn add_dot2(&mut self, p: Point, exposure: usize) {
        let start_index = self.out_index;
        for _ in 0..exposure {
            self.add_point(p.0 as u16, p.1 as u16);
        }
        self.parts.push((start_index, self.out_index));
    }

    pub fn add_line(&mut self, a: Point, b: Point) {
        let start_index = self.out_index;
        for (x, y) in Bresenham::new(a, b) {
            self.add_point(x as u16, y as u16)
        }
        self.parts.push((start_index, self.out_index));
    }

    pub fn add_closed_polygon(&mut self, points: &[Point]) {
        let start_index = self.out_index;
        closed_polygon_to_lines(points, |a, b| {
            for (x, y) in Bresenham::new(a, b) {
                self.add_point(x as u16, y as u16)
            }
        });
        self.parts.push((start_index, self.out_index));
    }

    pub fn add_open_polygon(&mut self, points: &[Point]) {
        let start_index = self.out_index;
        open_polygon_to_lines(points, |a, b| {
            for (x, y) in Bresenham::new(a, b) {
                self.add_point(x as u16, y as u16)
            }
        });
        self.parts.push((start_index, self.out_index));
    }

    pub fn add_circle(&mut self, center: Point, radius: f32, nodes: usize) {
        let mut points: Vec<Point> = Vec::with_capacity(nodes);
        for i in 0..nodes {
            let phi = (i as f32 / nodes as f32) * core::f32::consts::PI * 2.0;
            let x = roundf(center.0 as f32 + libm::sinf(phi) * radius) as isize;
            let y = roundf(center.1 as f32 + libm::cosf(phi) * radius) as isize;
            points.push((x, y));
        }
        self.add_closed_polygon(&points);
    }

    pub fn add_double_circle(&mut self, center: Point, radius: f32, nodes: usize) {
        let mut points: Vec<Point> = Vec::with_capacity(nodes * 2);
        for i in 0..nodes * 2 {
            let phi = (i as f32 / nodes as f32) * core::f32::consts::PI * 2.0;
            let x = roundf(center.0 as f32 + libm::sinf(phi) * radius) as isize;
            let y = roundf(center.1 as f32 + libm::cosf(phi) * radius) as isize;
            points.push((x, y));
        }
        self.add_closed_polygon(&points);
    }

    pub fn draw_font(
        &mut self,
        drawing: &Drawing,
        scale: f32,
        translate_x: isize,
        translate_y: isize,
    ) {
        for line in drawing.lines {
            let line: Vec<Point> = line
                .iter()
                .map(|p| {
                    let x = roundf(p.0 * scale) as isize + translate_x;
                    let y = roundf(-p.1 * scale) as isize + translate_y;
                    (x, y)
                })
                .collect();
            self.add_open_polygon(&line);
        }
    }
}
