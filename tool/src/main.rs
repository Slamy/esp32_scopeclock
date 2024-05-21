use std::iter::Sum;
use std::ops::{Add, SubAssign};

use svg::node::element::path::{Command, Data, Position};
use svg::node::element::tag::Path;
use svg::parser::Event;

#[derive(Debug, Clone, Copy)]
struct PointF32(f32, f32);

impl Add<PointF32> for PointF32 {
    type Output = PointF32;
    fn add(self, other: PointF32) -> PointF32 {
        return PointF32(self.0 + other.0, self.1 + other.1);
    }
}

impl SubAssign<PointF32> for PointF32 {
    fn sub_assign(&mut self, rhs: PointF32) {
        self.0 -= rhs.0;
        self.1 -= rhs.1;
    }
}
/*
impl Sum<PointF32> for PointF32 {
    fn sum<I: Iterator<Item = PointF32>>(iter: I) -> Self {
        return PointF32(0.0, 0.0);
    }
}
*/
#[derive(Debug)]

struct Drawing {
    lines: Vec<Vec<PointF32>>,
}

impl Drawing {
    fn center(&mut self) {
        //let len = self.lines.iter().flatten().count() as f32;

        // according to https://stackoverflow.com/questions/64186095/max-of-f64-in-rust
        let max_x = self
            .lines
            .iter()
            .flatten()
            .max_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap()
            .0;

        let min_x = self
            .lines
            .iter()
            .flatten()
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .unwrap()
            .0;

        let max_y = self
            .lines
            .iter()
            .flatten()
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
            .1;

        let min_y = self
            .lines
            .iter()
            .flatten()
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .unwrap()
            .1;

        //let max_y: f32 = self.lines.iter().flatten().map(|x| x.0).max();
        //let sum_y: f32 = self.lines.iter().flatten().map(|x| x.1).sum();

        let avg_x = (max_x + min_x) / 2_f32;
        let avg_y = (max_y + min_y) / 2_f32;
        let avg = PointF32(avg_x, avg_y);

        println!("center at {} {}", avg_x, avg_y);

        for line in self.lines.iter_mut() {
            for pos in line.iter_mut() {
                *pos -= avg;
            }
        }
    }

    fn print(&self) {
        //println!("\nprint of {:?}", self.lines.len());
        for (index, line) in self.lines.iter().enumerate() {
            println!("{}: {:?}", index, line);
        }
    }
}

fn main() {
    let path = "numbers.svg";
    let mut content = String::new();

    let mut drawings: Vec<Drawing> = Vec::new();

    for event in svg::open(path, &mut content).unwrap() {
        match event {
            Event::Tag(Path, _, attributes) => {
                //println!("\n\n{:?}\n{:?}", x, attributes);

                let mut drawing = Drawing { lines: Vec::new() };
                let mut line: Vec<PointF32> = Vec::new();
                let mut pen: PointF32 = PointF32(0.0, 0.0);

                let data = attributes.get("d").unwrap();
                let data = Data::parse(data).unwrap();
                for command in data.iter() {
                    match command {
                        Command::Move(Position::Relative, params) => {
                            //println!("Move {:?} {:?}", x, params);

                            //let mut iterator = params.chunks(2);

                            // First one is a move
                            //let i = iterator.next().unwrap();
                            //println!("Move {:?}", (i[0], i[1]));

                            line = if line.is_empty() == false {
                                drawing.lines.push(line);
                                Vec::new()
                            } else {
                                line
                            };

                            //line.push(newpen);
                            //let pen = pen + PointF32(i[0], i[1]);

                            // Following are implicit lines
                            for i in params.chunks(2) {
                                //println!("Implicit Line {:?}", (i[0], i[1]));
                                pen = pen + PointF32(i[0], i[1]);
                                line.push(pen);
                            }
                        }
                        Command::Line(Position::Relative, params) => {
                            //println!("Line {:?} {:?}", x, params);

                            for i in params.chunks(2) {
                                //println!("Line {:?}", (i[0], i[1]));
                                pen = pen + PointF32(i[0], i[1]);
                                line.push(pen);
                            }
                        }
                        Command::Line(Position::Absolute, params) => {
                            //println!("Line {:?} {:?}", x, params);
                            for i in params.chunks(2) {
                                //println!("Line {:?}", (i[0], i[1]));
                                pen = PointF32(i[0], i[1]);
                                line.push(pen);
                            }
                        }
                        Command::HorizontalLine(Position::Relative, params) => {
                            //println!("HorizontalLine {:?} {:?}", x, params);
                            for x_rel in params.iter() {
                                //println!("Line {:?}", (i[0], i[1]));
                                pen.0 += *x_rel;
                                line.push(pen);
                                //println!("{:?}", newpen);
                            }
                        }
                        Command::HorizontalLine(Position::Absolute, params) => {
                            //println!("HorizontalLine {:?} {:?}", x, params);
                            for x_rel in params.iter() {
                                //println!("Line {:?}", (i[0], i[1]));
                                pen.0 = *x_rel;
                                line.push(pen);
                                //println!("{:?}", newpen);
                            }
                        }
                        Command::VerticalLine(Position::Relative, params) => {
                            //println!("VerticalLine {:?} {:?}", x, params);
                            for y_rel in params.iter() {
                                //println!("Line {:?}", (i[0], i[1]));
                                pen.1 += *y_rel;
                                line.push(pen);
                                //println!("{:?}", newpen);
                            }
                        }
                        Command::VerticalLine(Position::Absolute, params) => {
                            //println!("VerticalLine {:?} {:?}", x, params);
                            for y_rel in params.iter() {
                                //println!("Line {:?}", (i[0], i[1]));
                                pen.1 = *y_rel;
                                line.push(pen);
                                //println!("{:?}", newpen);
                            }
                        }
                        x => {
                            panic!("Unknown! {:?}", x);
                        }
                    }
                }
                /*
                if pen.0 != 0.0 {
                    line.push(pen);
                }
                */
                if line.is_empty() == false {
                    drawing.lines.push(line);
                }

                //drawing.print();
                //drawing.center();
                //drawing.print();
                //return;
                drawings.push(drawing);
            }
            x => {
                println!("Ignored {:?}", x)
            }
        }
    }

    // sort by y coordinate
    drawings.sort_by(|x, y| x.lines[0][0].1.partial_cmp(&y.lines[0][0].1).unwrap());

    println!("len {}", drawings.len());

    drawings.iter_mut().for_each(|x| {
        println!("handle drawing");
        //x.print();
        x.center();
        //println!("Centered:");
        x.print()
    });

    println!("{:?}", drawings);
    //println!("{:?}", );
    println!("Hello, world!");
}
