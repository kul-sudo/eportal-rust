use crate::body::AdaptationSkill;
use crate::constants::*;
use crate::Virus;
use crate::{ADAPTATION_SKILLS_COUNT, VIRUSES_COUNT};
use serde_derive::Deserialize;
use std::collections::HashSet;
use std::fs::read_to_string;
use std::mem::variant_count;
use toml::from_str;

#[derive(Deserialize)]
struct Config {
    average_vision_distance: f32,
    average_energy: f32,
    average_division_threshold: f32,
}

#[derive(Deserialize)]
struct Viruses {
    speedvirus_first_generation_infection_chance: f32,
    speedvirus_speed_decrease: f32,
    speedvirus_energy_spent_for_healing: f32,
    speedvirus_heal_energy: f32,

    visionvirus_first_generation_infection_chance: f32,
    visionvirus_vision_distance_decrease: f32,
    visionvirus_energy_spent_for_healing: f32,
    visionvirus_heal_energy: f32,
}

#[derive(Deserialize)]
struct Data {
    body: Config,
    viruses: Viruses,
}

pub fn config_setup() {
    let contents = match read_to_string(CONFIG_FILE_NAME) {
        Ok(contents) => contents,
        Err(_) => {
            eprintln!("The config file hasn't been found.");
            panic!();
        }
    };

    let config: Data = match from_str(&contents) {
        Ok(config) => config,
        Err(_) => {
            eprintln!("Unable to find the config file.");
            panic!();
        }
    };

    let body = config.body;
    let viruses = config.viruses;
    unsafe {
        AVERAGE_VISION_DISTANCE = body.average_vision_distance;
        AVERAGE_ENERGY = body.average_energy;
        AVERAGE_DIVISION_THRESHOLD = body.average_division_threshold;
        SPEEDVIRUS_FIRST_GENERATION_INFECTION_CHANCE =
            viruses.speedvirus_first_generation_infection_chance;
        SPEEDVIRUS_SPEED_DECREASE = viruses.speedvirus_speed_decrease;
        SPEEDVIRUS_ENERGY_SPENT_FOR_HEALING = viruses.speedvirus_energy_spent_for_healing;
        SPEEDVIRUS_HEAL_ENERGY = viruses.speedvirus_heal_energy;

        VISIONVIRUS_FIRST_GENERATION_INFECTION_CHANCE =
            viruses.visionvirus_first_generation_infection_chance;
        VISIONVIRUS_VISION_DISTANCE_DECREASE = viruses.visionvirus_vision_distance_decrease;
        VISIONVIRUS_ENERGY_SPENT_FOR_HEALING = viruses.visionvirus_energy_spent_for_healing;
        VISIONVIRUS_HEAL_ENERGY = viruses.visionvirus_heal_energy;
    };
}

pub fn enum_consts() -> (HashSet<usize>, HashSet<usize>) {
    // Skills
    let mut variant_count_ = variant_count::<AdaptationSkill>();
    unsafe {
        ADAPTATION_SKILLS_COUNT = variant_count_;
    }
    let all_skills = (0..variant_count_).collect::<HashSet<_>>();

    // Viruses
    variant_count_ = variant_count::<Virus>();
    unsafe {
        VIRUSES_COUNT = variant_count_;
    }
    let all_viruses = (0..variant_count_).collect::<HashSet<_>>();

    (all_skills, all_viruses)
}