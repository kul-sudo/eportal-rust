#![feature(core_intrinsics)]
#![feature(more_float_constants)]

mod body;
mod constants;
mod plant;

use body::*;
use constants::*;
use plant::{randomly_spawn_plant, Plant};

use std::{
    collections::{HashMap, HashSet},
    env::consts::OS,
    f32::consts::SQRT_2,
    intrinsics::unlikely,
    thread::sleep,
    time::{Duration, Instant},
};

use macroquad::{
    camera::{set_camera, Camera2D},
    color::{GREEN, RED, WHITE},
    input::{is_mouse_button_pressed, mouse_position},
    math::{vec2, Rect, Vec2},
    miniquad::{window::set_fullscreen, MouseButton},
    shapes::{draw_circle, draw_circle_lines, draw_line, draw_rectangle, draw_triangle},
    text::{draw_text, measure_text},
    window::{next_frame, screen_height, screen_width, Conf},
};
use rand::{rngs::ThreadRng, seq::SliceRandom, thread_rng, Rng};

/// Adjust the coordinates according to the borders.
macro_rules! adjusted_coordinates {
    ($pos:expr, $area_size:expr) => {
        (
            ($pos.x * MAX_ZOOM)
                .max($area_size.x / MAX_ZOOM / 2.0)
                .min($area_size.x * (1.0 - 1.0 / (2.0 * MAX_ZOOM))),
            ($pos.y * MAX_ZOOM)
                .max($area_size.y / MAX_ZOOM / 2.0)
                .min($area_size.y * (1.0 - 1.0 / (2.0 * MAX_ZOOM))),
        )
    };
}

/// Used for getting specific values with deviations.
#[macro_export]
macro_rules! get_with_deviation {
    ($value:expr, $rng:expr) => {{
        let part = $value * DEVIATION;
        $rng.gen_range($value - part..$value + part)
    }};
}

/// Set the camera zoom to where the mouse cursor is.
fn get_zoom_target(camera: &mut Camera2D, area_size: Vec2) {
    let (x, y) = mouse_position();
    let (target_x, target_y) = adjusted_coordinates!(Vec2 { x, y }, area_size);

    camera.target = vec2(target_x, target_y);
    camera.zoom = vec2(MAX_ZOOM / area_size.x * 2.0, MAX_ZOOM / area_size.y * 2.0);
    set_camera(camera);
}

/// Reset the camera zoom.
fn default_camera(camera: &mut Camera2D, area_size: Vec2) {
    camera.target = vec2(area_size.x / 2.0, area_size.y / 2.0);
    camera.zoom = vec2(MIN_ZOOM / area_size.x * 2.0, MIN_ZOOM / area_size.y * 2.0);
    set_camera(camera);
}

// fn get_nearest_plant_for_body(plants: &[Plant], body: &Body) -> Option<(f32, (usize, Plant))> {
//     let (plant_id, plant) = plants
//         .iter()
//         .enumerate()
//         .min_by_key(|(_, plant)| plant.pos.distance(body.pos) as i16)?;
//     Some((plant.pos.distance(body.pos), (plant_id, *plant)))
// }

// fn get_nearest_body_for_body<'a>(
//     bodies: &'a HashMap<usize, Body<'a>>,
//     body: &Body,
// ) -> Option<(f32, (usize, &'a Body<'a>))> {
//     let (body_id, closest_body) = bodies.iter().min_by_key(|(_, enemy_body)| {
//         distance(vec![enemy_body.x, enemy_body.y], vec![body.x, body.y]) as isize
//     })?;
//     Some((
//         distance(vec![closest_body.x, closest_body.y], vec![body.x, body.y]),
//         (*body_id, closest_body),
//     ))
// }

fn window_conf() -> Conf {
    Conf {
        window_title: "eportal".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    // Make the window fullscreen on Linux: for some reason, when the application has been built,
    // Arch Linux apparently doesn't have enough time to make it fullscreen
    if OS == "linux" {
        set_fullscreen(true);
        sleep(Duration::from_secs(1));
        next_frame().await;
    }

    let area_size = Vec2 {
        x: screen_width() * OBJECT_RADIUS,
        y: screen_height() * OBJECT_RADIUS,
    };

    let mut bodies: HashMap<usize, Body> = HashMap::with_capacity(BODIES_N);
    let mut plants: Vec<Plant> = Vec::with_capacity(PLANTS_N);

    let rng = &mut thread_rng();

    // Spawn the bodies
    for i in 1..BODIES_N {
        randomly_spawn_body(
            &mut bodies,
            area_size,
            if i >= BODY_EATERS_N {
                EatingStrategy::Plants
            } else {
                EatingStrategy::Bodies
            },
            rng,
        );
    }

    // Spawn the plants
    for _ in 0..PLANTS_N {
        randomly_spawn_plant(&mut bodies, &mut plants, rng, area_size)
    }

    let mut camera = Camera2D::from_display_rect(Rect::new(0.0, 0.0, area_size.x, area_size.y));
    default_camera(&mut camera, area_size);

    let mut zoom_mode = false;

    // The timer needed for the FPS
    let mut last_updated = Instant::now();

    loop {
        // Handle the left mouse button click for zooming in/out
        if unlikely(is_mouse_button_pressed(MouseButton::Left)) {
            if zoom_mode {
                default_camera(&mut camera, area_size);
            } else {
                get_zoom_target(&mut camera, area_size);
            }

            zoom_mode = !zoom_mode
        }

        if zoom_mode {
            get_zoom_target(&mut camera, area_size);
        }

        // Spawn a plant in a random place with a specific chance
        if unlikely(rng.gen_range(0.0..1.0) > 1.0 - PLANT_SPAWN_CHANCE) {
            randomly_spawn_plant(&mut bodies, &mut plants, rng, area_size)
        }

        let mut bodies_to_iter = bodies.iter_mut().collect::<Vec<_>>();
        bodies_to_iter.shuffle(rng);

        // Whether enough time has passed to draw a new frame
        let is_draw_mode =
            last_updated.elapsed().as_millis() >= Duration::from_secs(1 / FPS).as_millis();

        // Due to certain borrowing rules, it's impossible to modify the `bodies` hashmap during the loop,
        // so it'll be done after it
        let mut bodies_to_delete: HashSet<usize> = HashSet::with_capacity(bodies_to_iter.len());
        let mut eaten_plants: Vec<Plant> = Vec::with_capacity(plants.len());

        for (body_id, body) in bodies_to_iter {
            if body.energy.is_sign_negative() {
                match body.death_time {
                    Some(timestamp) => {
                        if timestamp.elapsed().as_secs() >= CROSS_LIFESPAN {
                            bodies_to_delete.insert(*body_id);
                        }
                    }
                    None => {
                        body.death_time = Some(Instant::now());
                    }
                }
                continue;
            }

            // The mass is proportional to the energy; to keep the mass up, energy is spent
            body.energy -= ENERGY_SPEND_CONST_FOR_MASS * body.energy
                + ENERGY_SPEND_CONST_FOR_IQ * body.iq
                + ENERGY_SPEND_CONST_FOR_VISION * body.vision_distance;

            body.status = Status::Sleeping;

            match body.eating_strategy {
                EatingStrategy::Bodies => {}
                EatingStrategy::Plants => {
                    let mut distance_to_closest_plant = 0.0;
                    let closest_plant = plants
                        .iter()
                        .enumerate()
                        .filter(|(_, plant)| {
                            body.pos.distance(plant.pos) <= body.vision_distance
                                && !eaten_plants.contains(plant)
                        })
                        .min_by(|(_, x), (_, y)| {
                            distance_to_closest_plant = body.pos.distance(y.pos);
                            body.pos
                                .distance(x.pos)
                                .partial_cmp(&distance_to_closest_plant)
                                .unwrap()
                        });

                    if let Some((closest_plant_id, closest_plant)) = closest_plant {
                        body.pos = Vec2 {
                            x: body.pos.x
                                + ((closest_plant.pos.x - body.pos.x) * body.speed)
                                    / distance_to_closest_plant,
                            y: body.pos.y
                                + ((closest_plant.pos.y - body.pos.y) * body.speed)
                                    / distance_to_closest_plant,
                        };

                        if body.pos.distance(closest_plant.pos) <= body.speed {
                            eaten_plants.push(*closest_plant);
                            body.energy += PLANT_HP;
                        }
                        body.status = Status::FollowingPlant(*closest_plant);
                    }
                }
            }
        }

        for body in &bodies_to_delete {
            bodies.remove(body);
        }
        bodies_to_delete.clear();

        plants.retain(|plant| !eaten_plants.contains(plant));

        if is_draw_mode {
            for plant in &plants {
                draw_plant!(plant);
            }

            for (body_id, body) in &bodies {
                draw_circle_lines(
                    body.pos.x,
                    body.pos.y,
                    body.vision_distance,
                    2.0,
                    body.color,
                );
                let to_display = body.energy;
                draw_text(
                    &to_display.to_string(),
                    body.pos.x
                        - measure_text(&to_display.to_string(), None, BODY_INFO_FONT_SIZE, 1.0)
                            .width
                            / 2.0,
                    body.pos.y - OBJECT_RADIUS - MIN_GAP,
                    BODY_INFO_FONT_SIZE as f32,
                    WHITE,
                );

                draw_body!(body);
            }
            // draw_text(&format!("zoom {}", zoom), 10.0, 20.0, 20.0, WHITE);

            // if zoom_mode {
            //     let mouse_position = mouse_position();
            //     let (x, y) = adjusted_coordinates!(
            //         mouse_position.0 + 25.0,
            //         mouse_position.1 - 25.0,
            //         area_size
            //     );
            //     draw_text("zoomed in", x, y, 10.0 * MAX_ZOOM, WHITE)
            // }
        }

        if is_draw_mode {
            last_updated = Instant::now();
            next_frame().await;
        }
    }
}
