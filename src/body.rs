use std::{
    collections::HashMap,
    time::{Instant, SystemTime, UNIX_EPOCH},
};

use macroquad::{
    color::{Color, GREEN},
    math::{Vec2, Vec3},
    rand::gen_range,
};
use rand::{random, rngs::StdRng, Rng};

use crate::{constants::*, get_with_deviation, time_since_unix_epoch};

#[derive(Clone, Copy, PartialEq)]
pub enum Status {
    FollowingTarget((u128, Vec2)),
    EscapingBody((u128, u16)),
    Dead(Instant),
    Walking(Vec2),
    Idle,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EatingStrategy {
    Bodies,
    Plants,
}

#[derive(Clone, Copy, PartialEq)]
pub struct Body {
    pub pos: Vec2,
    pub energy: f32,
    pub speed: f32,
    pub vision_distance: f32,
    pub eating_strategy: EatingStrategy,
    /// The body procreates after a specific level of energy has been reached.
    pub division_threshold: f32,
    pub iq: u8,
    pub color: Color,
    pub status: Status,
    /// When the body died due to a lack of energy if it did die in the first place.
    pub body_type: u16,
    pub lifespan: f32,
}

#[allow(clippy::too_many_arguments)]
impl Body {
    pub fn new(
        pos: Vec2,
        energy: f32,
        speed: f32,
        vision_distance: f32,
        eating_strategy: EatingStrategy,
        division_threshold: f32,
        iq: u8,
        color: Color,
        is_first_generation: bool,
        rng: &mut StdRng,
        body_type: u16,
    ) -> Self {
        Body {
            pos,
            energy: if is_first_generation {
                get_with_deviation!(energy, rng)
            } else {
                energy / 2.0
            } - VISION_DISTANCE_BIRTH_ENERGY_SPENT * vision_distance
                - SPEED_BIRTH_ENERGY_SPENT * speed,
            speed: get_with_deviation!(speed, rng),
            vision_distance: get_with_deviation!(vision_distance, rng),
            eating_strategy,
            division_threshold: get_with_deviation!(division_threshold, rng),
            iq: if is_first_generation {
                iq
            } else if rng.gen_range(0.0..1.0) < IQ_CHANGE_CHANCE {
                match random::<bool>() {
                    true => {
                        if iq == MAX_IQ {
                            iq
                        } else {
                            iq + 1
                        }
                    }
                    false => {
                        if iq == MIN_IQ {
                            iq
                        } else {
                            iq + 1
                        }
                    }
                }
            } else {
                iq
            },
            color,
            status: Status::Idle,
            body_type,
            lifespan: LIFESPAN,
        }
    }

    pub fn is_alive(&self) -> bool {
        !matches!(self.status, Status::Dead(..))
    }

    pub fn wrap(&mut self, area_size: Vec2) {
        if self.pos.x > area_size.x {
            self.pos.x = MIN_GAP;
        } else if self.pos.x < 0.0 {
            self.pos.x = area_size.x - MIN_GAP;
        }

        if self.pos.y > area_size.y {
            self.pos.y = MIN_GAP;
        } else if self.pos.y < 0.0 {
            self.pos.y = area_size.y - MIN_GAP;
        }
    }
}

/// Generate a random position until it suits certain creteria.
pub fn randomly_spawn_body(
    bodies: &mut HashMap<u128, Body>,
    area_size: Vec2,
    eating_strategy: EatingStrategy,
    rng: &mut StdRng,
    body_type: usize,
) {
    let mut pos = Vec2::default();

    // Make sure the position is far enough from the rest of the bodies and the borders of the area
    while {
        pos.x = rng.gen_range(0.0..area_size.x);
        pos.y = rng.gen_range(0.0..area_size.y);
        (pos.x <= OBJECT_RADIUS + MIN_GAP || pos.x >= area_size.x - OBJECT_RADIUS - MIN_GAP)
            || (pos.y <= OBJECT_RADIUS + MIN_GAP || pos.y >= area_size.y - OBJECT_RADIUS - MIN_GAP)
            || bodies
                .values()
                .any(|body| body.pos.distance(pos) < OBJECT_RADIUS * 2.0 + MIN_GAP)
    } {}

    // Make sure the color is different enough
    let real_color_gap = COLOR_GAP / ((BODIES_N + 1) as f32).powf(1.0 / 3.0);

    let mut color = Color::from_rgba(
        gen_range(COLOR_MIN, COLOR_MAX),
        gen_range(COLOR_MIN, COLOR_MAX),
        gen_range(COLOR_MIN, COLOR_MAX),
        255,
    );

    let green_rgb = Vec3 {
        x: GREEN.r,
        y: GREEN.g,
        z: GREEN.b,
    };

    while bodies.values().any(|body| {
        let current_body_rgb = Vec3 {
            x: body.color.r,
            y: body.color.g,
            z: body.color.b,
        };
        current_body_rgb.distance(green_rgb) < real_color_gap
            || current_body_rgb.distance(Vec3 {
                x: color.r,
                y: color.g,
                z: color.b,
            }) < real_color_gap
    }) {
        color = Color::from_rgba(
            gen_range(COLOR_MIN, COLOR_MAX),
            gen_range(COLOR_MIN, COLOR_MAX),
            gen_range(COLOR_MIN, COLOR_MAX),
            255,
        )
    }

    bodies.insert(
        time_since_unix_epoch!(),
        Body::new(
            pos,
            match eating_strategy {
                EatingStrategy::Bodies => BODY_EATER_AVERAGE_ENERGY,
                EatingStrategy::Plants => PLANT_EATER_AVERAGE_ENERGY,
            },
            AVERAGE_SPEED,
            AVERAGE_VISION_DISTANCE,
            eating_strategy,
            match eating_strategy {
                EatingStrategy::Bodies => BODY_EATER_AVERAGE_DIVISION_THRESHOLD,
                EatingStrategy::Plants => PLANT_EATER_AVERAGE_DIVISION_THRESHOLD,
            },
            0,
            color,
            true,
            rng,
            body_type as u16,
        ),
    );
}

#[macro_export]
macro_rules! draw_body {
    ($body:expr) => {
        let side_length_half = OBJECT_RADIUS / SQRT_2;

        if $body.is_alive() {
            match $body.eating_strategy {
                EatingStrategy::Bodies => {
                    let side_length = side_length_half * 2.0;
                    draw_rectangle(
                        $body.pos.x - side_length_half,
                        $body.pos.y - side_length_half,
                        side_length,
                        side_length,
                        $body.color,
                    )
                }

                EatingStrategy::Plants => {
                    draw_circle($body.pos.x, $body.pos.y, OBJECT_RADIUS, $body.color)
                }
            }
        } else {
            draw_line(
                $body.pos.x - side_length_half,
                $body.pos.y - side_length_half,
                $body.pos.x + side_length_half,
                $body.pos.y + side_length_half,
                2.0,
                $body.color,
            );

            draw_line(
                $body.pos.x + side_length_half,
                $body.pos.y - side_length_half,
                $body.pos.x - side_length_half,
                $body.pos.y + side_length_half,
                2.0,
                $body.color,
            )
        }
    };
}
