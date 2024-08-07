#![allow(internal_features)]
#![feature(core_intrinsics)]
#![feature(more_float_constants)]
#![feature(variant_count)]
#![feature(let_chains)]

mod body;
mod cells;
mod condition;
mod constants;
mod cross;
mod plant;
mod smart_drawing;
mod user_constants;
mod utils;
mod zoom;

use body::*;
use cells::*;
use condition::*;
use constants::*;
use cross::*;
use plant::*;
use user_constants::*;
use utils::*;
use zoom::*;

use std::{
    collections::{HashMap, HashSet},
    intrinsics::unlikely,
    mem::variant_count,
    time::{Duration, Instant},
};

use macroquad::{
    camera::Camera2D,
    color::WHITE,
    input::{
        is_key_down, is_key_pressed, is_mouse_button_pressed,
        mouse_position, KeyCode,
    },
    math::{Rect, Vec2},
    miniquad::{window::set_fullscreen, MouseButton},
    prelude::vec2,
    shapes::{draw_circle_lines, draw_line},
    window::{next_frame, screen_height, screen_width, Conf},
};
use rand::{rngs::StdRng, seq::IteratorRandom, Rng, SeedableRng};

pub static mut TOTAL_SKILLS_COUNT: usize = 0;
pub static mut VIRUSES_COUNT: usize = 0;
pub static mut UI_SHOW_PROPERTIES_N: usize = 0;

fn window_conf() -> Conf {
    Conf {
        window_title: "eportal".to_owned(),
        fullscreen: true,
        ..Default::default()
    }
}

#[macroquad::main(window_conf)]
async fn main() {
    assert_eq!(Condition::ALL.len(), variant_count::<Condition>());
    assert_eq!(Virus::ALL.len(), variant_count::<Virus>());
    assert_eq!(Skill::ALL.len(), variant_count::<Skill>());
    assert_eq!(PlantKind::ALL.len(), variant_count::<PlantKind>());

    config_setup();

    // A workaround for Linux
    if cfg!(target_os = "linux") {
        set_fullscreen(true);
        std::thread::sleep(Duration::from_secs(1));
        next_frame().await;
    }

    let area_size = vec2(
        // OBJECT_RADIUS is equal to one pixel when unzoomed
        screen_width() * OBJECT_RADIUS,
        screen_height() * OBJECT_RADIUS,
    );

    // Needed for randomness
    let mut rng = StdRng::from_rng(&mut rand::thread_rng()).unwrap();

    // Calculations
    let area_space = area_size.x * area_size.y;

    unsafe {
        PLANTS_N = (PLANTS_DENSITY * area_space).round() as usize;
        PLANTS_N_FOR_ONE_STEP =
            (PLANT_SPAWN_CHANCE * area_space).round() as usize;
    }

    let cells = generate_cells(&area_size);

    // Camera
    let mut camera = Camera2D::from_display_rect(Rect::new(
        0.0,
        0.0,
        area_size.x,
        area_size.y,
    ));

    default_camera(&mut camera, &area_size);

    // Info
    let mut info = Info {
        body_info:      true,
        evolution_info: EvolutionInfo {
            show:         false,
            last_updated: None,
            last_info:    None,
        },
    };

    // Evolution stuff
    let mut condition: Option<(Condition, (Instant, Duration))> =
        None;

    let mut bodies: HashMap<BodyId, Body> =
        HashMap::with_capacity(unsafe { BODIES_N });
    let mut plants: HashMap<Cell, HashMap<PlantId, Plant>> =
        HashMap::with_capacity(cells.rows * cells.columns);
    let mut crosses: HashMap<Cell, HashMap<CrossId, Cross>> =
        HashMap::with_capacity(cells.rows * cells.columns);

    for i in 0..cells.rows {
        for j in 0..cells.columns {
            plants.insert(Cell { i, j }, HashMap::new());
            crosses.insert(Cell { i, j }, HashMap::new());
        }
    }

    // Spawn the bodies
    for i in 0..unsafe { BODIES_N } {
        Body::randomly_spawn_body(
            &mut bodies,
            &area_size,
            if unsafe { PASSIVE_CHANCE } == 1.0
                || rng.gen_range(0.0..1.0)
                    <= unsafe { PASSIVE_CHANCE }
            {
                EatingStrategy::Passive
            } else {
                EatingStrategy::Active
            },
            i + 1,
            &mut rng,
        );
    }

    // Needs to be handled manually to avoid extracting all plants out of the cells
    let mut plants_n = 0;

    // Spawn the plants
    for _ in 0..unsafe { PLANTS_N } {
        Plant::randomly_spawn_plant(
            &bodies,
            &mut plants,
            &area_size,
            &cells,
            &mut rng,
        );

        plants_n += 1;
    }

    // Zoom
    let rect_size = vec2(
        screen_width() / MAX_ZOOM * OBJECT_RADIUS,
        screen_height() / MAX_ZOOM * OBJECT_RADIUS,
    );

    let mut zoom = generate_zoom_struct(&area_size);

    // Needed for the FPS
    let mut last_updated = Instant::now();

    loop {
        // Handle interactions
        if unlikely(is_key_pressed(KeyCode::Escape)) {
            std::process::exit(0);
        }

        if unlikely(is_mouse_button_pressed(MouseButton::Left)) {
            if zoom.zoomed {
                default_camera(&mut camera, &area_size);
                zoom.mouse_pos = None;
            } else {
                zoom.rect = None;
                zoom.extended_rect = None;
                zoom.rect = None;
            }

            zoom.zoomed = !zoom.zoomed
        }

        if unlikely(is_key_pressed(KeyCode::Key1)) {
            if zoom.zoomed {
                info.body_info = !info.body_info;
            }
        }

        if unlikely(is_key_pressed(KeyCode::Key2)) {
            info.evolution_info.show = !info.evolution_info.show;
            info.evolution_info.last_updated = Some(Instant::now());
        }

        if zoom.zoomed {
            // There's no reason to zoom in again if the mouse position hasn't been changed
            let current_mouse_pos = Vec2::from(mouse_position());
            match zoom.mouse_pos {
                Some(mouse_pos) => {
                    if mouse_pos != current_mouse_pos {
                        zoom.mouse_pos = Some(current_mouse_pos);
                        get_zoom_target(
                            &mut camera,
                            &area_size,
                            &mut zoom,
                            &rect_size,
                        );
                    }
                }
                None => {
                    zoom.mouse_pos = Some(current_mouse_pos);
                    get_zoom_target(
                        &mut camera,
                        &area_size,
                        &mut zoom,
                        &rect_size,
                    );
                }
            }
        }

        let mut new_bodies: HashMap<BodyId, Body> = HashMap::new();

        let mut removed_plants: HashMap<PlantId, Vec2> =
            HashMap::new();
        let mut removed_bodies: HashSet<BodyId> = HashSet::new();
        let mut removed_crosses: HashMap<CrossId, Vec2> =
            HashMap::new();

        Condition::update_condition(&mut condition, &mut rng);

        // Remove plants
        let n_to_remove = (plants_n as f32
            * (unsafe { PLANT_DIE_CHANCE }
                + if condition.is_some_and(|(condition, _)| {
                    condition == Condition::Drought
                }) {
                    (unsafe { PLANT_DIE_CHANCE })
                        * DROUGHT_PLANT_DIE_CHANCE_MULTIPLIER
                } else {
                    0.0
                })) as usize;

        for _ in 0..n_to_remove {
            loop {
                // Pick a random cell and remove a random plant from it
                let random_cell =
                    plants.iter().choose(&mut rng).unwrap().0;

                if let Some((random_plant_id, random_plant)) = plants
                    .get(random_cell)
                    .unwrap()
                    .iter()
                    .choose(&mut rng)
                {
                    if !removed_plants.contains_key(random_plant_id) {
                        removed_plants.insert(
                            *random_plant_id,
                            random_plant.pos,
                        );

                        plants_n -= 1;
                        break;
                    }
                }
            }
        }

        // Spawn a plant in a random place with a specific chance
        let n_to_add = unsafe { PLANTS_N_FOR_ONE_STEP }
            + if condition.is_some_and(|(condition, _)| {
                condition == Condition::Rain
            }) {
                (unsafe { PLANTS_N_FOR_ONE_STEP } as f32
                    * RAIN_PLANTS_N_FOR_ONE_STEP_MULTIPLIER)
                    as usize
            } else {
                0
            };

        for _ in 0..n_to_add {
            Plant::randomly_spawn_plant(
                &bodies,
                &mut plants,
                &area_size,
                &cells,
                &mut rng,
            );

            plants_n += 1;
        }

        // Whether enough time has passed to draw a new frame
        let is_draw_mode = last_updated.elapsed().as_millis()
            >= Duration::from_secs(1 / FPS).as_millis();

        for (body_id, body) in unsafe {
            &mut (*(&mut bodies as *mut HashMap<BodyId, Body>))
        } {
            body.handle_viruses();
            body.handle_lifespan();

            // Handle if dead to become a cross
            if body.energy < unsafe { MIN_ENERGY }
                || body_id.elapsed().as_secs_f32() > body.lifespan
            {
                body.status = Status::Cross;
                removed_bodies.insert(*body_id);

                continue;
            }

            if body.handle_energy(body_id, &mut removed_bodies) {
                continue;
            }

            // Escape
            let mut chasers = body.followed_by.clone();

            if !chasers.is_empty() {
                if body
                    .skills
                    .contains(&Skill::PrioritizeFasterChasers)
                    && chasers.iter().any(|(_, other_body)| {
                        other_body.speed > body.speed
                    })
                {
                    chasers.retain(|_, other_body| {
                        other_body.speed > body.speed
                    })
                }

                if let Some((
                    closest_chasing_body_id,
                    closest_chasing_body,
                )) = chasers.iter().min_by(|(_, a), (_, b)| {
                    body.pos
                        .distance(a.pos)
                        .total_cmp(&body.pos.distance(b.pos))
                }) {
                    body.set_status(
                        Status::EscapingBody(
                            *closest_chasing_body_id,
                            closest_chasing_body.body_type,
                        ),
                        &body_id,
                        &cells,
                        &mut bodies,
                        unsafe {
                            &mut (*(&mut crosses
                                as *mut HashMap<
                                    Cell,
                                    HashMap<CrossId, Cross>,
                                >))
                        },
                        &mut plants,
                    );

                    let distance_to_closest_chasing_body =
                        body.pos.distance(closest_chasing_body.pos);

                    body.pos.x -= (closest_chasing_body.pos.x
                        - body.pos.x)
                        * (body.speed
                            / distance_to_closest_chasing_body);
                    body.pos.y -= (closest_chasing_body.pos.y
                        - body.pos.y)
                        * (body.speed
                            / distance_to_closest_chasing_body);

                    body.wrap(&area_size);

                    continue;
                }
            }

            // Eating
            let mut food: Option<FoodInfo> = None;

            // Find the closest plant
            let mut visible_crosses: HashMap<&CrossId, &Cross> =
                HashMap::new();

            get_visible!(
                body,
                cells,
                unsafe {
                    &mut (*(&mut crosses
                        as *mut HashMap<
                            Cell,
                            HashMap<CrossId, Cross>,
                        >))
                },
                visible_crosses
            );

            // Find the closest cross
            match visible_crosses
                .iter()
                .filter(|(_, cross)| {
                    body.handle_eat_crosses_of_my_type(cross)
                        && body.handle_alive_when_arrived_cross(cross)
                        && body.handle_profitable_when_arrived_cross(
                            cross,
                        )
                        && body.handle_avoid_new_viruses_cross(cross)
                        && body.handle_will_arrive_first_cross(
                            body_id, cross,
                        )
                        && body.handle_do_not_compete_with_relatives(
                            &body_id,
                            &cross.followed_by,
                        )
                })
                .min_by(|(_, a), (_, b)| {
                    body.pos
                        .distance(a.pos)
                        .partial_cmp(&body.pos.distance(b.pos))
                        .unwrap()
                }) {
                Some((closest_cross_id, closest_cross)) => {
                    food = Some(FoodInfo {
                        id:        **closest_cross_id,
                        food_type: ObjectType::Cross,
                        pos:       closest_cross.pos,
                        energy:    closest_cross.energy,
                        viruses:   Some(&closest_cross.viruses),
                    });
                }
                None => {
                    let mut visible_plants: HashMap<
                        &PlantId,
                        &Plant,
                    > = HashMap::new();

                    get_visible!(body, cells, plants, visible_plants);

                    let filtered_visible_plants = visible_plants
                        .iter()
                        .filter(|(plant_id, plant)| {
                            !removed_plants.contains_key(plant_id)
                            && body.handle_alive_when_arrived_plant(plant)
                            && body.handle_profitable_when_arrived_plant(plant)
                            && body.handle_do_not_compete_with_relatives(
                                &body_id,
                                &plant.followed_by
                            )
                            && body.handle_will_arrive_first_plant(
                                body_id,
                                plant,
                            )
                        }).collect::<Vec<_>>();

                    let mut closest_plant = body.find_closest_plant(
                        &filtered_visible_plants,
                        PlantKind::Banana,
                    );

                    if closest_plant.is_none() {
                        closest_plant = body.find_closest_plant(
                            &filtered_visible_plants,
                            PlantKind::Grass,
                        );
                    }

                    match closest_plant {
                        Some((closest_plant_id, closest_plant)) => {
                            food = Some(FoodInfo {
                                id:        ***closest_plant_id,
                                food_type: ObjectType::Plant,
                                pos:       closest_plant.pos,
                                energy:    closest_plant
                                    .get_contained_energy(),
                                viruses:   None,
                            })
                        }
                        None => {
                            // Find the closest body
                            if let Some((closest_body_id, closest_body)) =  unsafe {
                                &(*(&bodies as *const HashMap<BodyId, Body>))
                            }
                                .iter()
                                .filter(|(other_body_id, other_body)| {
                                    body.body_type != other_body.body_type &&
                                    &body_id != other_body_id
                                    && body.energy > other_body.energy
                                    && body.pos.distance(other_body.pos)
                                    <= body.vision_distance
                                    && !removed_bodies.contains(other_body_id)
                                    && body.handle_alive_when_arrived_body(
                                        other_body,
                                    )
                                    && body.handle_profitable_when_arrived_body(
                                        other_body,
                                    )
                                    && body.handle_avoid_new_viruses_body(other_body)
                                    && body.handle_will_arrive_first_body(
                                        body_id,
                                        other_body,
                                    )
                                    && body.handle_do_not_compete_with_relatives(
                                        &body_id,
                                        &other_body.followed_by
                                    )
                                })
                                .min_by(|(_, a), (_, b)| {
                                    body.pos
                                        .distance(a.pos)
                                        .partial_cmp(&body.pos.distance(b.pos))
                                        .unwrap()
                                })
                            {
                                food = Some(FoodInfo {
                                    id:        *closest_body_id,
                                    food_type: ObjectType::Body,
                                    pos:       closest_body.pos,
                                    energy:    closest_body.energy,
                                    viruses: Some(&closest_body.viruses)
                                })
                            }
                        }
                    }
                }
            }

            if let Some(food) = food {
                let distance_to_food = body.pos.distance(food.pos);
                if distance_to_food <= body.speed {
                    body.energy += food.energy;
                    body.pos = food.pos;

                    match food.food_type {
                        ObjectType::Body => {
                            body.get_viruses(&food.viruses.unwrap());
                            removed_bodies.insert(food.id);
                        }
                        ObjectType::Cross => {
                            body.get_viruses(&food.viruses.unwrap());
                            removed_crosses.insert(food.id, food.pos);
                        }
                        ObjectType::Plant => {
                            removed_plants.insert(food.id, food.pos);
                            plants_n -= 1;
                        }
                    }
                } else {
                    Body::followed_by_cleanup(
                        &body_id,
                        &cells,
                        &mut bodies,
                        unsafe {
                            &mut (*(&mut crosses
                                as *mut HashMap<
                                    Cell,
                                    HashMap<CrossId, Cross>,
                                >))
                        },
                        &mut plants,
                        Some(&food),
                    );

                    match food.food_type {
                        ObjectType::Body => {
                            unsafe {
                                &mut (*(&mut bodies
                                    as *mut HashMap<BodyId, Body>))
                            }
                            .get_mut(&food.id)
                            .unwrap()
                            .followed_by
                            .insert(*body_id, body.clone());
                        }
                        ObjectType::Cross => {
                            crosses
                                .get_mut(
                                    &cells.get_cell_by_pos(&food.pos),
                                )
                                .unwrap()
                                .get_mut(&food.id)
                                .unwrap()
                                .followed_by
                                .insert(*body_id, body.clone());
                        }
                        ObjectType::Plant => {
                            plants
                                .get_mut(
                                    &cells.get_cell_by_pos(&food.pos),
                                )
                                .unwrap()
                                .get_mut(&food.id)
                                .unwrap()
                                .followed_by
                                .insert(*body_id, body.clone());
                        }
                    }

                    body.status = Status::FollowingTarget(
                        food.id,
                        food.pos,
                        food.food_type,
                    );

                    body.pos.x += (food.pos.x - body.pos.x)
                        * (body.speed / distance_to_food);
                    body.pos.y += (food.pos.y - body.pos.y)
                        * (body.speed / distance_to_food);

                    continue;
                }
            }

            // Procreate
            if body.handle_procreation(
                body_id,
                &mut new_bodies,
                &mut removed_bodies,
                &mut rng,
            ) {
                continue;
            }

            body.handle_walking_idle(
                &body_id,
                &cells,
                &mut bodies,
                &mut crosses,
                &mut plants,
                &area_size,
                &mut rng,
            );
        }

        for (cross_id, cross_pos) in &removed_crosses {
            crosses
                .get_mut(&cells.get_cell_by_pos(cross_pos))
                .unwrap()
                .remove(cross_id);
        }

        for crosses in crosses.values_mut() {
            crosses.retain(|_, cross| {
                cross.timestamp.elapsed().as_secs()
                    <= unsafe { CROSS_LIFESPAN }
            })
        }

        for body_id in &removed_bodies {
            Body::followed_by_cleanup(
                &body_id,
                &cells,
                &mut bodies,
                &mut crosses,
                &mut plants,
                None,
            );

            let body = bodies.get(&body_id).unwrap();

            if let Status::Cross = body.status {
                crosses
                    .get_mut(&cells.get_cell_by_pos(&body.pos))
                    .unwrap()
                    .insert(*body_id, Cross::new(&body));
            }

            bodies.remove(body_id);
        }

        for (new_body_id, new_body) in new_bodies {
            bodies.insert(new_body_id, new_body);
        }

        for (plant_id, plant_pos) in &removed_plants {
            plants
                .get_mut(&cells.get_cell_by_pos(plant_pos))
                .unwrap()
                .remove(plant_id);
        }

        if is_draw_mode {
            if !is_key_down(KeyCode::Space) {
                if zoom.zoomed {
                    for plant in Plant::get_plants_to_draw(
                        &cells,
                        &zoom,
                        &plants,
                        &removed_plants,
                        plants_n,
                    ) {
                        plant.draw();
                    }

                    for body in bodies.values() {
                        let drawing_strategy =
                            body.get_drawing_strategy(&zoom);

                        if info.body_info {
                            if drawing_strategy.vision_distance {
                                draw_circle_lines(
                                    body.pos.x,
                                    body.pos.y,
                                    body.vision_distance,
                                    2.0,
                                    body.color,
                                );
                            }

                            if drawing_strategy.target_line {
                                if let Status::FollowingTarget(
                                    _,
                                    target_pos,
                                    _,
                                ) = body.status
                                {
                                    draw_line(
                                        body.pos.x,
                                        body.pos.y,
                                        target_pos.x,
                                        target_pos.y,
                                        2.0,
                                        WHITE,
                                    );
                                }
                            }
                        }

                        if drawing_strategy.body {
                            body.draw();
                        }

                        if drawing_strategy.vision_distance
                            && info.body_info
                        {
                            body.draw_info();
                        }
                    }
                } else {
                    for body in bodies.values() {
                        body.draw();
                    }

                    for cell in plants.values() {
                        for plant in cell.values() {
                            plant.draw();
                        }
                    }
                }

                for cell in crosses.values() {
                    for cross in cell.values() {
                        cross.draw(&zoom);
                    }
                }

                last_updated = Instant::now();
            }

            if info.evolution_info.show {
                show_evolution_info(
                    &zoom,
                    &area_size,
                    &mut info,
                    plants_n,
                    bodies.len(),
                    &condition,
                );
            }

            if unsafe { SHOW_FPS } {
                show_fps(&zoom);
            }

            next_frame().await;
        }
    }
}
