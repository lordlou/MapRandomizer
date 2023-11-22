pub mod game_data;
pub mod traverse;
pub mod randomize;
pub mod patch;
pub mod spoiler_map;
pub mod seed_repository;
pub mod customize;

use game_data::{StartLocation, HubLocation};
use patch::Rom;
use pyo3::{prelude::*, types::PyDict};
use rand::{SeedableRng, RngCore};
use randomize::{Randomization, SpoilerLog, escape_timer, randomize_doors, ItemPlacementStyle, ItemPriorityGroup, ItemMarkers, RandomizationState, ItemLocationState, FlagLocationState, SaveLocationState, MotherBrainFight};
use traverse::TraverseResult;
use crate::{
    game_data::{GameData, Map, IndexedVec, Item, NodeId, RoomId, ObstacleMask},
    randomize::{Randomizer, DifficultyConfig, VertexInfo, SaveAnimals},
    traverse::{GlobalState, LocalState, traverse, is_bireachable},
    patch::make_rom,
};
use std::{path::{Path, PathBuf}, mem::transmute};
use reqwest::blocking::get;
use anyhow::{Context, Result};
use serde_derive::Deserialize;
use url::Url;
use hashbrown::{HashMap, HashSet};

#[pyclass]
#[derive(Deserialize, Clone)]
struct Preset {
    #[pyo3(get)]
    name: String,
    shinespark_tiles: usize,
    resource_multiplier: f32,
    escape_timer_multiplier: f32,
    phantoon_proficiency: f32,
    draygon_proficiency: f32,
    ridley_proficiency: f32,
    botwoon_proficiency: f32,
    #[pyo3(get)]
    tech: Vec<String>,
    #[pyo3(get)]
    notable_strats: Vec<String>,
}

#[pyclass]
#[derive(Clone)]
struct PresetData {
    #[pyo3(get)]
    preset: Preset,
    #[pyo3(get)]
    tech_setting: Vec<(String, bool)>,
    #[pyo3(get)]
    implicit_tech: HashSet<String>,
    #[pyo3(get)]
    notable_strat_setting: Vec<(String, bool)>,
}

fn init_presets(

    presets: Vec<Preset>,
    game_data: &GameData,
    ignored_notable_strats: &HashSet<String>,
    implicit_tech: &HashSet<String>,
) -> Vec<PresetData> {
    let mut out: Vec<PresetData> = Vec::new();
    let mut cumulative_tech: HashSet<String> = HashSet::new();
    let mut cumulative_strats: HashSet<String> = HashSet::new();

    // Tech which is currently not used by any strat in logic, so we avoid showing on the website:
    let ignored_tech: HashSet<String> = [
        "canGrappleClip",
        "canShinesparkWithReserve",
        //"canRiskPermanentLossOfAccess",
        "canIceZebetitesSkip",
        "canSpeedZebetitesSkip",
        "canRemorphZebetiteSkip",
        "canEscapeMorphLocation", // Special internal tech for "vanilla map" option
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();
    for tech in &ignored_tech {
        if !game_data.tech_isv.index_by_key.contains_key(tech) {
            panic!("Unrecognized ignored tech \"{tech}\"");
        }
    }
    for tech in implicit_tech {
        if !game_data.tech_isv.index_by_key.contains_key(tech) {
            panic!("Unrecognized implicit tech \"{tech}\"");
        }
        if ignored_tech.contains(tech) {
            panic!("Tech is both ignored and implicit: \"{tech}\"");
        }
    }

    let all_notable_strats: HashSet<String> = game_data
        .links
        .iter()
        .filter_map(|x| x.notable_strat_name.clone())
        .collect();
    if !ignored_notable_strats.is_subset(&all_notable_strats) {
        let diff: Vec<String> = ignored_notable_strats
            .difference(&all_notable_strats)
            .cloned()
            .collect();
        panic!("Unrecognized ignored notable strats: {:?}", diff);
    }

    let visible_tech: Vec<String> = game_data
        .tech_isv
        .keys
        .iter()
        .filter(|&x| !ignored_tech.contains(x) && !implicit_tech.contains(x))
        .cloned()
        .collect();
    let visible_tech_set: HashSet<String> = visible_tech.iter().cloned().collect();

    let visible_notable_strats: HashSet<String> = all_notable_strats
        .iter()
        .filter(|&x| !ignored_notable_strats.contains(x))
        .cloned()
        .collect();

    cumulative_tech.extend(implicit_tech.iter().cloned());
    for preset in presets {
        for tech in &preset.tech {
            if cumulative_tech.contains(tech) {
                panic!("Tech \"{tech}\" appears in presets more than once.");
            }
            if !visible_tech_set.contains(tech) {
                panic!(
                    "Unrecognized tech \"{tech}\" appears in preset {}",
                    preset.name
                );
            }
            cumulative_tech.insert(tech.clone());
        }
        let mut tech_setting: Vec<(String, bool)> = Vec::new();
        for tech in implicit_tech {
            tech_setting.push((tech.clone(), true));
        }
        for tech in &visible_tech {
            tech_setting.push((tech.clone(), cumulative_tech.contains(tech)));
        }

        for strat_name in &preset.notable_strats {
            if cumulative_strats.contains(strat_name) {
                panic!("Notable strat \"{strat_name}\" appears in presets more than once.");
            }
            cumulative_strats.insert(strat_name.clone());
        }
        let mut notable_strat_setting: Vec<(String, bool)> = Vec::new();
        for strat_name in &visible_notable_strats {
            notable_strat_setting
                .push((strat_name.clone(), cumulative_strats.contains(strat_name)));
        }

        out.push(PresetData {
            preset: preset,
            tech_setting: tech_setting,
            implicit_tech: implicit_tech.clone(),
            notable_strat_setting: notable_strat_setting,
        });
    }
    for tech in &visible_tech_set {
        if !cumulative_tech.contains(tech) {
            panic!("Tech \"{tech}\" not found in any preset.");
        }
    }

    if !visible_notable_strats.is_subset(&cumulative_strats) {
        let diff: Vec<String> = visible_notable_strats
            .difference(&cumulative_strats)
            .cloned()
            .collect();
        panic!("Notable strats not found in any preset: {:?}", diff);
    }
    if !cumulative_strats.is_subset(&visible_notable_strats) {
        let diff: Vec<String> = cumulative_strats
            .difference(&visible_notable_strats)
            .cloned()
            .collect();
        panic!("Unrecognized notable strats in presets: {:?}", diff);
    }

    out
}

fn get_ignored_notable_strats() -> HashSet<String> {
    [
        "Suitless Botwoon Kill",
        "Maridia Bug Room Frozen Menu Bridge",
        "Breaking the Maridia Tube Gravity Jump",
        "Metroid Room 1 PB Dodge Kill (Left to Right)",
        "Metroid Room 1 PB Dodge Kill (Right to Left)",
        "Metroid Room 2 PB Dodge Kill (Bottom to Top)",
        "Metroid Room 3 PB Dodge Kill (Left to Right)",
        "Metroid Room 3 PB Dodge Kill (Right to Left)",
        "Metroid Room 4 Three PB Kill (Top to Bottom)",
        "Metroid Room 4 Six PB Dodge Kill (Bottom to Top)",
        "Metroid Room 4 Three PB Dodge Kill (Bottom to Top)",
        "Partial Covern Ice Clip",
        "Mickey Mouse Crumble Jump IBJ",
        "G-Mode Morph Breaking the Maridia Tube Gravity Jump", // not usable because of canRiskPermanentLossOfAccess
        "Mt. Everest Cross Room Jump through Top Door", // currently unusable because of obstacleCleared requirement
    ]
    .iter()
    .map(|x| x.to_string())
    .collect()
}

fn get_implicit_tech() -> HashSet<String> {
    [
        "canSpecialBeamAttack",
        "canMidAirMorph",
        "canTurnaroundSpinJump",
        "canStopOnADime",
        "canUseGrapple",
        "canEscapeEnemyGrab",
        "canDownBack",
        "canRiskPermanentLossOfAccess",
    ]
    .into_iter()
    .map(|x| x.to_string())
    .collect()
}

#[derive(Clone)]
#[pyclass]
pub struct Options {
    #[pyo3(get, set)]
    preset: usize,
    #[pyo3(get, set)]
    techs: Vec<String>,
    #[pyo3(get, set)]
    strats: Vec<String>,
    #[pyo3(get, set)]
    shinespark_tiles: usize,
    #[pyo3(get, set)]
    resource_multiplier: f32,
    #[pyo3(get, set)]
    gate_glitch_leniency: i32,
    #[pyo3(get, set)]
    phantoon_proficiency: f32,
    #[pyo3(get, set)]
    draygon_proficiency: f32,
    #[pyo3(get, set)]
    ridley_proficiency: f32,
    #[pyo3(get, set)]
    botwoon_proficiency: f32,
    #[pyo3(get, set)]
    escape_timer_multiplier: f32,
    #[pyo3(get, set)]
    randomized_start: bool,
    #[pyo3(get, set)]
    save_animals: bool,
    #[pyo3(get, set)]
    early_save: bool,
    #[pyo3(get, set)]
    objectives: u8,
    #[pyo3(get, set)]
    doors_mode: u8,
    #[pyo3(get, set)]
    filler_items: String,
    #[pyo3(get, set)]
    supers_double: bool,
    #[pyo3(get, set)]
    mother_brain_fight: u8,
    #[pyo3(get, set)]
    escape_enemies_cleared: bool,
    #[pyo3(get, set)]
    escape_refill: bool,
    #[pyo3(get, set)]
    escape_movement_items: bool,
    #[pyo3(get, set)]
    mark_map_stations: bool,
    #[pyo3(get, set)]
    transition_letters: bool, 
    #[pyo3(get, set)]
    item_markers: u8,
    #[pyo3(get, set)]
    item_dots_disappear: bool,
    #[pyo3(get, set)]
    all_items_spawn: bool,
    #[pyo3(get, set)]
    acid_chozo: bool,
    #[pyo3(get, set)]
    fast_elevators: bool,
    #[pyo3(get, set)]
    fast_doors: bool,
    #[pyo3(get, set)]
    fast_pause_menu: bool, 
    #[pyo3(get, set)]
    respin: bool, 
    #[pyo3(get, set)]
    infinite_space_jump: bool,
    #[pyo3(get, set)]
    momentum_conservation: bool,
    #[pyo3(get, set)]
    disable_walljump: bool,
    #[pyo3(get, set)]
    maps_revealed: bool,
    #[pyo3(get, set)]
    map_layout: usize,
    #[pyo3(get, set)]
    ultra_low_qol: bool,
    #[pyo3(get, set)]
    skill_assumptions_preset: String,
    #[pyo3(get, set)]
    item_progression_preset: String,
    #[pyo3(get, set)]
    quality_of_life_preset: usize,
}

#[pymethods]
impl Options{
    #[new]
    pub fn new( preset: usize,
                techs: Vec<String>,
                strats: Vec<String>,
                shinespark_tiles: usize,
                resource_multiplier: f32,
                gate_glitch_leniency: i32,
                phantoon_proficiency: f32,
                draygon_proficiency: f32,
                ridley_proficiency: f32,
                botwoon_proficiency: f32,
                escape_timer_multiplier: f32,
                randomized_start: bool,
                save_animals: bool,
                early_save: bool,
                objectives: u8,
                doors_mode: u8,
                filler_items: String,
                supers_double: bool,
                mother_brain_fight: u8,
                escape_enemies_cleared: bool,
                escape_refill: bool,
                escape_movement_items: bool,
                mark_map_stations: bool,
                transition_letters: bool,
                item_markers: u8,
                item_dots_disappear: bool,
                all_items_spawn: bool,
                acid_chozo: bool,
                fast_elevators: bool,
                fast_doors: bool,
                fast_pause_menu: bool,
                respin: bool,
                infinite_space_jump: bool,
                momentum_conservation: bool,
                disable_walljump: bool,
                maps_revealed: bool,
                map_layout: usize,
                ultra_low_qol: bool,
                skill_assumptions_preset: String,
                item_progression_preset: String,
                quality_of_life_preset: usize) -> Self {
        Options { 
            preset,
            techs,
            strats,
            shinespark_tiles,
            resource_multiplier,
            gate_glitch_leniency,
            phantoon_proficiency,
            draygon_proficiency,
            ridley_proficiency,
            botwoon_proficiency,
            escape_timer_multiplier,
            randomized_start,
            save_animals,
            early_save,
            objectives,
            doors_mode,
            filler_items,
            supers_double,
            mother_brain_fight,
            escape_enemies_cleared,
            escape_refill,
            escape_movement_items,
            mark_map_stations,
            transition_letters,
            item_markers,
            item_dots_disappear,
            all_items_spawn,
            acid_chozo,
            fast_elevators,
            fast_doors,
            fast_pause_menu,
            respin,
            infinite_space_jump,
            momentum_conservation,
            disable_walljump,
            maps_revealed,
            map_layout,
            ultra_low_qol,
            skill_assumptions_preset,
            item_progression_preset,
            quality_of_life_preset
        }
    }
}

/* 
fn make_box<T>(src: &pyo3::PyAny) -> pyo3::PyResult<Box<T>>
where
    T: for<'a> pyo3::FromPyObject<'a>
{
    src.extract().map(Box::new)
}*/

fn make_optional_box<T>(src: &pyo3::PyAny) -> pyo3::PyResult<Option<Box<T>>>
where
    T: for<'a> pyo3::FromPyObject<'a> 
{
    src.extract().map(|val| Some(Box::new(val))).or(Ok(None))
}

impl GlobalState {
    pub fn remove(&mut self, item: Item, game_data: &GameData) {
        self.items[item as usize] = false;
        match item {
            Item::Missile => {
                if self.max_missiles > 0 {
                    self.max_missiles -= 5;
                }
            }
            Item::Super => {
                if self.max_supers > 0 {
                    self.max_supers -= 5;
                }
            }
            Item::PowerBomb => {
                if self.max_power_bombs > 0 {
                    self.max_power_bombs -= 5;
                }
            }
            Item::ETank => {
                if self.max_energy > 99 {
                    self.max_energy -= 100;
                }
            }
            Item::ReserveTank => {
                if self.max_reserves > 0 {
                 self.max_reserves -= 100;
                }
            }
            _ => {}
        }
        self.weapon_mask = game_data.get_weapon_mask(&self.items);
    }
}


#[derive(Clone)]
#[pyclass]
pub struct APCollectionState {
    #[pyo3(get)]
    randomization_state: RandomizationState,
    //local_states: Vec<Option<LocalState>>,
    //#[pyo3(get)]
    //cost: Vec<f32>,
    ap_randomizer: Option<Box<APRandomizer>>,
}

/*
#[pymethods]
impl RandomizationState {
    #[new]
    fn new(randomizer: Randomizer) -> Self {
        let initial_item_location_state = ItemLocationState {
            placed_item: None,
            collected: false,
            reachable: false,
            bireachable: false,
            bireachable_vertex_id: None,
        };
        let initial_flag_location_state = FlagLocationState {
            bireachable: false,
            bireachable_vertex_id: None,
        };
        // let item_precedence: Vec<Item> =
        //    randomizer.get_item_precedence(&randomizer.difficulty_tiers[0].item_priorities, &mut rng);
        // info!("Item precedence: {:?}", item_precedence);
        RandomizationState {
            step_num: 1,
            item_precedence: Vec::new(),
            item_location_state: vec![
                initial_item_location_state;
                randomizer.game_data.item_locations.len()
            ],
            flag_location_state: vec![
                initial_flag_location_state;
                randomizer.game_data.flag_locations.len()
            ],
            items_remaining: randomizer.initial_items_remaining.clone(),
            global_state: initial_global_state,
            done: false,
            debug_data: None,
            previous_debug_data: None,
            key_visited_vertices: HashSet::new(),
        }
    }
}*/

#[pymethods]
impl APCollectionState{
    #[new]
    pub fn new(
        #[pyo3(from_py_with = "make_optional_box")]
        ap_randomizer: Option<Box<APRandomizer>>) -> Self {
        let initial_item_location_state = ItemLocationState {
            placed_item: None,
            collected: false,
            reachable: false,
            bireachable: false,
            bireachable_vertex_id: None,
        };
        let initial_flag_location_state = FlagLocationState {
            bireachable: false,
            bireachable_vertex_id: None,
        };
        let initial_save_location_state = SaveLocationState { bireachable: false };
        let global_state = match &ap_randomizer {
            Some(ap_r) => {
                    let rando = &ap_r.randomizer;
                    let items = vec![false; rando.game_data.item_isv.keys.len() - 2];
                    let weapon_mask = rando.game_data.get_weapon_mask(&items);
                    GlobalState {
                        tech: rando.get_tech_vec(0),
                        notable_strats: rando.get_strat_vec(0),
                        items: items,
                        flags: rando.get_initial_flag_vec(),
                        max_energy: 99,
                        max_reserves: 0,
                        max_missiles: 0,
                        max_supers: 0,
                        max_power_bombs: 0,
                        weapon_mask: weapon_mask,
                        shine_charge_tiles: rando.difficulty_tiers[0].shine_charge_tiles,
                    }
                },
            None => GlobalState {
                tech: Vec::new(),
                notable_strats: Vec::new(),
                items: Vec::new(),
                flags: Vec::new(),
                max_energy: 99,
                max_reserves: 0,
                max_missiles: 0,
                max_supers: 0,
                max_power_bombs: 0,
                weapon_mask: 0,
                shine_charge_tiles: 0.0,
            },
        };
        let randomizer = &ap_randomizer.as_ref().unwrap().randomizer;
        let randomization_state = RandomizationState {
            step_num: 1,
            start_location: ap_randomizer.as_ref().unwrap().start_location.clone(),
            hub_location: ap_randomizer.as_ref().unwrap().hub_location.clone(),
            item_precedence: Vec::new(),
            item_location_state: vec![
                initial_item_location_state;
                randomizer.game_data.item_locations.len()
            ],
            flag_location_state: vec![
                initial_flag_location_state;
                randomizer.game_data.flag_locations.len()
            ],
            save_location_state: vec![
                initial_save_location_state;
                randomizer.game_data.item_locations.len()
            ],
            items_remaining: randomizer.initial_items_remaining.clone(),
            global_state: global_state,
            debug_data: None,
            previous_debug_data: None,
            key_visited_vertices: HashSet::new(),
        };
        APCollectionState { 
            randomization_state,
            ap_randomizer
        }
    }

    pub fn copy(&self) -> APCollectionState {
        /*let mut ap_collection_state = APCollectionState::new(self.ap_randomizer.clone());
        ap_collection_state.randomization_state = self.randomization_state.clone();
        ap_collection_state*/
        self.clone()
    }
    /*fn can_traverse(&mut self, ap_region_from_id: usize, strats_links: HashMap<String, Vec<usize>>) -> bool {
        let src_id = self.ap_randomizer.as_ref().unwrap().regions_map_reverse[ap_region_from_id];
        let src_local_state = self.local_states[src_id].unwrap();
        let mut result = false;
        for link_vec_id in strats_links.values() {
            let mut strat_result = true;
            for link_id in link_vec_id {
                let link = &self.ap_randomizer.as_ref().unwrap().randomizer.links[*link_id];
                let dst_id = link.to_vertex_id;
                let dst_old_cost = self.cost[dst_id];
                //println!("link.requirement: {:?}", link.requirement);
                if let Some(dst_new_local_state) = apply_requirement(
                    &link.requirement,
                    &self.global_state,
                    src_local_state,
                    false,
                    &self.ap_randomizer.as_ref().unwrap().randomizer.difficulty_tiers[0],
                ) {
                    //println!("link.requirement: passed");
                    let dst_new_cost = compute_cost(dst_new_local_state, &self.global_state);
                    if dst_new_cost < dst_old_cost {
                        self.local_states[dst_id] = Some(dst_new_local_state);
                        self.cost[dst_id] = dst_new_cost;
                    }
                }
                else {
                    strat_result = false;
                }
            }
            result |= strat_result;
        }
        result
    }*/

    pub fn add_item(&mut self, item: usize) {
        self.randomization_state.global_state.collect(unsafe { transmute(item) }, &self.ap_randomizer.as_ref().unwrap().randomizer.game_data);
    }

    pub fn remove_item(&mut self, item: usize) {
        self.randomization_state.global_state.remove(unsafe { transmute(item) }, &self.ap_randomizer.as_ref().unwrap().randomizer.game_data);
    }

    pub fn add_flag(&mut self, flag: usize) {
        self.randomization_state.global_state.flags[flag] = true;
    }

    pub fn remove_flag(&mut self, flag: usize) {
        self.randomization_state.global_state.flags[flag] = false;
    }

    pub fn __deepcopy__(&self, _memo: &PyDict) -> Self {self.clone()}
}

fn get_difficulty_config(options: &Options, preset_data: &Vec<PresetData>, game_data: &GameData) -> DifficultyConfig {
    
    let preset;
    if options.preset < preset_data.len() {
        let pd = preset_data[options.preset].clone();
        //let tech_set: HashSet<String> = pd.preset.tech.iter().cloned().collect();
        //let strat_set: HashSet<String> = pd.preset.notable_strats.iter().cloned().collect();
        let mut tech_vec: Vec<String> = Vec::new();
        for (tech, enabled) in &pd.tech_setting {
            if *enabled {
                tech_vec.push(tech.clone());
            }
        }
        tech_vec.sort();

        let mut strat_vec: Vec<String> = vec![]; //= app_data.ignored_notable_strats.iter().cloned().collect();
        for (strat, enabled) in &pd.notable_strat_setting {
            if *enabled {
                strat_vec.push(strat.clone());
            }
        }
        strat_vec.sort();

        preset = Preset {
            name: pd.preset.name,
            shinespark_tiles: pd.preset.shinespark_tiles,
            resource_multiplier: pd.preset.resource_multiplier,
            escape_timer_multiplier: pd.preset.escape_timer_multiplier,
            phantoon_proficiency: pd.preset.phantoon_proficiency,
            draygon_proficiency: pd.preset.draygon_proficiency,
            ridley_proficiency: pd.preset.ridley_proficiency,
            botwoon_proficiency: pd.preset.botwoon_proficiency,
            tech: tech_vec,
            notable_strats: strat_vec,
        }
    }
    else {
        preset = Preset {
            name: "Custom".to_string(),
            shinespark_tiles: options.shinespark_tiles,
            resource_multiplier: options.resource_multiplier,
            escape_timer_multiplier: options.escape_timer_multiplier,
            phantoon_proficiency: options.phantoon_proficiency,
            draygon_proficiency: options.draygon_proficiency,
            ridley_proficiency: options.ridley_proficiency,
            botwoon_proficiency: options.botwoon_proficiency,
            tech: options.techs.clone(),
            notable_strats: options.strats.clone(),
            }
    }

    DifficultyConfig {
        tech: preset.tech,
        notable_strats: preset.notable_strats,
        shine_charge_tiles: preset.shinespark_tiles as f32,
        progression_rate: randomize::ProgressionRate::Normal,
        filler_items: vec![Item::Missile],
        early_filler_items: vec![],
        item_placement_style: ItemPlacementStyle::Neutral,
        item_priorities: vec![ItemPriorityGroup {
            name: "Default".to_string(),
            items: game_data.item_isv.keys.clone(),
        }],
        resource_multiplier: preset.resource_multiplier,
        gate_glitch_leniency: options.gate_glitch_leniency,
        escape_timer_multiplier: preset.escape_timer_multiplier,
        randomized_start: options.randomized_start,
        save_animals: match options.save_animals {
            false => SaveAnimals::No,
            true => SaveAnimals::Yes,
        },
        early_save: options.early_save,
        phantoon_proficiency: preset.phantoon_proficiency,
        draygon_proficiency: preset.draygon_proficiency,
        ridley_proficiency: preset.ridley_proficiency,
        botwoon_proficiency: preset.botwoon_proficiency,
        supers_double: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.supers_double,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        mother_brain_fight: match options.quality_of_life_preset {
            0 => MotherBrainFight::Vanilla,
            1 => MotherBrainFight::Skip,
            2 => unsafe { transmute(options.mother_brain_fight)},
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        escape_enemies_cleared: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.escape_enemies_cleared,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        escape_refill: options.escape_refill, 
        escape_movement_items: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.escape_movement_items,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        mark_map_stations: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.mark_map_stations,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        transition_letters: options.transition_letters,
        item_markers: match options.quality_of_life_preset {
            0 => ItemMarkers::Simple,
            1 => ItemMarkers::ThreeTiered,
            2 => unsafe { transmute(options.item_markers) },
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        item_dot_change: match options.item_dots_disappear {
            false => randomize::ItemDotChange::Fade,
            true => randomize::ItemDotChange::Disappear,
        },
        all_items_spawn: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.all_items_spawn,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        acid_chozo: options.acid_chozo,
        fast_elevators: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.fast_elevators,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        fast_doors: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.fast_doors,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        fast_pause_menu: options.fast_pause_menu,
        respin:  options.respin,
        infinite_space_jump:  options.infinite_space_jump,
        momentum_conservation: options.momentum_conservation,
        objectives: match options.objectives {
            0 => randomize::Objectives::Bosses,
            1 => randomize::Objectives::Minibosses,
            2 => randomize::Objectives::Metroids,
            3 => randomize::Objectives::Chozos,
            4 => randomize::Objectives::Pirates,
            _ => panic!("Unrecognized objectives: {}", options.objectives)
        },
        doors_mode: match options.doors_mode {
            0 => randomize::DoorsMode::Blue,
            1 => randomize::DoorsMode::Ammo,
            _ => panic!("Unrecognized doors_mode: {}", options.doors_mode)
        },
        disable_walljump:  options.disable_walljump,
        maps_revealed:  options.maps_revealed,
        map_layout: options.map_layout,
        ultra_low_qol:  options.ultra_low_qol,
        skill_assumptions_preset: Some("".to_string()/*options.skill_assumptions_preset*/),
        item_progression_preset: Some("".to_string()/*options.item_progression_preset*/),
        quality_of_life_preset: Some("Default".to_string()/*options.quality_of_life_preset*/),
        debug_options: None,
    }
}

#[pyclass]
#[derive(Clone)]
pub struct APRandomizer {
    #[pyo3(get)]
    randomizer: Randomizer,
    #[pyo3(get)]
    diff_settings: DifficultyConfig,   
    regions_map: Vec<usize>,
    #[pyo3(get)]
    preset_datas: Vec<PresetData>,
    seed: usize,
    start_location: StartLocation,
    hub_location: HubLocation
}

#[pymethods]
impl APRandomizer{
    #[new]
    pub fn new(game_data: &GameData, options: Options, seed: usize) -> Self {
        let presets: Vec<Preset> = serde_json::from_str(&game_data.read_to_string(Path::new(&"worlds/sm_map_rando/data/presets.json")).unwrap()).unwrap();
        let ignored_notable_strats = get_ignored_notable_strats();
        let implicit_tech = get_implicit_tech();
        let preset_datas = init_presets(presets, game_data, &ignored_notable_strats, &implicit_tech);
        let difficulty_tiers = vec![get_difficulty_config(&options, &preset_datas, game_data); 1];

        let (map_repo_filename, map_repo_url) = if difficulty_tiers[0].map_layout == 1 { 
            ("worlds/sm_map_rando/data/mapRepositoryTame.json",
            "https://storage.googleapis.com/super-metroid-map-rando/maps/session-2023-06-08T14:55:16.779895.pkl-small-71-subarea-balance-2/")
        }
        else {
            ("worlds/sm_map_rando/data/mapRepositoryWild.json",
            "https://storage.googleapis.com/super-metroid-map-rando/maps/session-2023-06-08T14:55:16.779895.pkl-small-64-subarea-balance-2/")
        };

        let binding = get_map_repository(game_data, map_repo_filename).unwrap();
        let map_repository_array = binding.as_slice();

        let mut map = if difficulty_tiers[0].map_layout == 0 {
            get_vanilla_map(game_data).unwrap()
        } else {   
            get_map(Path::new(map_repo_url), map_repository_array, TryInto::<usize>::try_into(seed).unwrap()).unwrap()
        };
        let diff_settings = difficulty_tiers[0].clone();
        let locked_doors = randomize_doors(&game_data, &map, &diff_settings, seed);

        let mut randomizer = Randomizer::new(Box::new(map), Box::new(locked_doors.clone()), Box::new(difficulty_tiers.clone()), Box::new(game_data.clone()));
        
        if diff_settings.map_layout != 0 && !diff_settings.randomized_start {
            let mut items_reachable = 0;
            let mut rng = {
                let mut rng_seed = [0u8; 32];
                rng_seed[..8].copy_from_slice(&seed.to_le_bytes());
                rand::rngs::StdRng::from_seed(rng_seed)
            };
            while items_reachable < 2 {
                let num_vertices = randomizer.game_data.vertex_isv.keys.len();
                let start_vertex_id = randomizer.game_data.vertex_isv.index_by_key[&(8, 5, 0)];
                let items = vec![false; randomizer.game_data.item_isv.keys.len() - 2];
                let weapon_mask = randomizer.game_data.get_weapon_mask(&items);
                let global = GlobalState {
                    tech: randomizer.get_tech_vec(0),
                    notable_strats: randomizer.get_strat_vec(0),
                    items: items,
                    flags: randomizer.get_initial_flag_vec(),
                    max_energy: 99,
                    max_reserves: 0,
                    max_missiles: 0,
                    max_supers: 0,
                    max_power_bombs: 0,
                    weapon_mask: weapon_mask,
                    shine_charge_tiles: randomizer.difficulty_tiers[0].shine_charge_tiles,
                };
                let forward = traverse(
                    &randomizer.links,
                    None,
                    &global,
                    LocalState::new(),
                    num_vertices,
                    start_vertex_id,
                    false,
                    &randomizer.difficulty_tiers[0],
                    &randomizer.game_data,
                    false
                );
                let reverse = traverse(
                    &randomizer.links,
                    None,
                    &global,
                    LocalState::new(),
                    num_vertices,
                    start_vertex_id,
                    true,
                    &randomizer.difficulty_tiers[0],
                    &randomizer.game_data,
                    false
                );

                for vertex_ids in randomizer.game_data.item_vertex_ids.iter() {    
                    for &v in vertex_ids {
                        if is_bireachable(
                                &global,
                                &forward.local_states[v],
                                &reverse.local_states[v]) {
                            items_reachable += 1;
                            if items_reachable >= 2 {
                                break;
                            }
                        }
                    }
                    if items_reachable >= 2 {
                        break;
                    }
                }
                if items_reachable < 2 {
                    let new_seed = (rng.next_u64() & 0xFFFFFFFF) as usize;
                    map = get_map(Path::new(map_repo_url), map_repository_array, TryInto::<usize>::try_into(new_seed).unwrap()).unwrap();
                    randomizer = Randomizer::new(Box::new(map), Box::new(locked_doors.clone()), Box::new(difficulty_tiers.clone()), Box::new(game_data.clone()));
                    println!("Not enough locations reachable from start ({:?}) trying new map.", items_reachable);
                }
            }
        }

        let (regions_map, _) = randomizer.game_data.get_regions_map();

        let mut rng = {
            let mut rng_seed = [0u8; 32];
            rng_seed[..8].copy_from_slice(&seed.to_le_bytes());
            rand::rngs::StdRng::from_seed(rng_seed)
        };
        let num_attempts_start_location = 350;
        let (start_location, hub_location) =
            randomizer.determine_start_location(1, num_attempts_start_location, &mut rng).unwrap();
        println!("start_location {}", start_location.name);

        APRandomizer { 
            randomizer,
            diff_settings,
            regions_map,
            preset_datas,
            seed,
            start_location,
            hub_location
        }
    }

    pub fn get_links_infos(&self) -> HashMap<(usize, usize), HashMap<String, Vec<usize>>> {
        let mut links: HashMap<(usize, usize), HashMap<String, Vec<usize>>> = HashMap::new();
        for (idx, link) in self.randomizer.links.iter().enumerate() {
            let key= (self.regions_map[link.from_vertex_id], self.regions_map[link.to_vertex_id]);
            links.entry(key).or_insert_with(HashMap::new).entry(link.strat_name.clone()).or_insert_with(Vec::new).push(idx);
        }
        links
    }

    pub fn get_link_requirement(&self, link_id: usize) -> String {
        format!("from:{} to:{} using {}: {:?}", 
        self.regions_map[self.randomizer.links[link_id].from_vertex_id], 
        self.regions_map[self.randomizer.links[link_id].to_vertex_id], 
            self.randomizer.links[link_id].strat_name, 
            self.randomizer.links[link_id].requirement)
    }

    pub fn update_reachability(&self, state: &mut RandomizationState, debug: bool)
        -> (Vec<bool>, Vec<bool>, Vec<bool>, TraverseResult, TraverseResult) {
        let num_vertices = self.randomizer.game_data.vertex_isv.keys.len();
        let mut bi_reachability = vec![false; num_vertices];
        let mut f_reachability = vec![false; num_vertices];
        let mut r_reachability = vec![false; num_vertices];
        // let start_vertex_id = self.randomizer.game_data.vertex_isv.index_by_key[&(8, 5, 0)]; // Landing site
        let start_vertex_id = self.randomizer.game_data.vertex_isv.index_by_key
            [&(state.hub_location.room_id, state.hub_location.node_id, 0)];
        let forward = traverse(
            &self.randomizer.links,
            None,
            &state.global_state,
            LocalState::new(),
            num_vertices,
            start_vertex_id,
            false,
            &self.randomizer.difficulty_tiers[0],
            self.randomizer.game_data.as_ref(),
            debug
        );
        let reverse = traverse(
            &self.randomizer.links,
            None,
            &state.global_state,
            LocalState::new(),
            num_vertices,
            start_vertex_id,
            true,
            &self.randomizer.difficulty_tiers[0],
            self.randomizer.game_data.as_ref(),
            debug
        );
        let mut bi_reachability_collapsed = Vec::new();
        let mut f_reachability_collapsed = Vec::new();
        let mut r_reachability_collapsed = Vec::new();
        let mut collapsed_count = 0;

        for i in 0..num_vertices {
            bi_reachability[i] = is_bireachable(
                                            &state.global_state,
                                            &forward.local_states[i],
                                            &reverse.local_states[i],
                                            );
            f_reachability[i] = forward.local_states[i].is_some();
            r_reachability[i] = reverse.local_states[i].is_some();

            if self.randomizer.game_data.vertex_isv.keys[i].2 == 0 {
                bi_reachability_collapsed.push(bi_reachability[i]);
                f_reachability_collapsed.push(f_reachability[i]);
                r_reachability_collapsed.push(r_reachability[i]);
                collapsed_count = collapsed_count + 1;
            }     
            else {
                bi_reachability_collapsed[collapsed_count - 1] |= bi_reachability[i];
                f_reachability_collapsed[collapsed_count - 1] |= f_reachability[i];
                r_reachability_collapsed[collapsed_count - 1] |= r_reachability[i];
            }                               
        }
        (bi_reachability_collapsed, f_reachability_collapsed, r_reachability_collapsed, forward, reverse)
    }
}

#[derive(Deserialize)]
pub struct MapRepository {
    pub map_array: Vec<String>,
}

fn get_map_repository(game_data: &GameData, path: &str) -> Result<Vec<String>> {
    let contents = game_data.read_to_string(Path::new(path))?;
    let map_array: Vec<String> = serde_json::from_str(&contents).unwrap();
    Ok(map_array)
}

fn get_map(base_path: & Path, filenames: &[String], seed: usize) -> Result<Map> {
    let idx = seed % filenames.len();
    let path: PathBuf = base_path.join(&filenames[idx]).with_extension("json");
    let url = Url::parse(path.to_str().unwrap()).unwrap();
    let response = get(url)
        .with_context(|| format!("Unable to fetch map file from {}", path.display()))?;
    let map: Map = response.json()
        .with_context(|| format!("Unable to parse map file at {}", path.display()))?;
    Ok(map)
}

fn get_vanilla_map(game_data: &GameData) -> Result<Map> {
    let path = Path::new("worlds/sm_map_rando/data/vanilla_map.json");
    let map_string = game_data.read_to_string(Path::new(&path))
        .with_context(|| format!("Unable to read map file at {}", path.display()))?;
    // info!("Map: {}", path.display());
    let map: Map = serde_json::from_str(&map_string)
        .with_context(|| format!("Unable to parse map file at {}", path.display()))?;
    Ok(map)
}

impl IntoPy<PyObject> for Box<APRandomizer> {
    fn into_py(self, py: Python<'_>) -> PyObject {
        (*self).into_py(py)
    }
}

impl IntoPy<PyObject> for Box<Map> {
    fn into_py(self, py: Python<'_>) -> PyObject {
        (*self).into_py(py)
    }
}

impl IntoPy<PyObject> for Box<GameData> {
    fn into_py(self, py: Python<'_>) -> PyObject {
        (*self).into_py(py)
    }
}

impl IntoPy<PyObject> for IndexedVec<String> {
    fn into_py(self, py: Python<'_>) -> PyObject {
        self.keys.into_py(py)
    }
}

impl IntoPy<PyObject> for IndexedVec<(RoomId, NodeId, ObstacleMask)> {
    fn into_py(self, py: Python<'_>) -> PyObject {
        self.keys.into_py(py)
    }
}

#[pymethods]
impl Randomizer {
    fn ap_get_vertex_info_by_id(&self, room_id: RoomId, node_id: NodeId) -> VertexInfo {
        self.get_vertex_info_by_id(room_id, node_id)
    }
}

#[pymethods]
impl GameData {
    fn get_regions_map(&self) -> (Vec<usize>, Vec<usize>) {
        let mut regions_map = Vec::new();
        let mut regions_map_reverse = Vec::new();
        let mut current_idx = 0;
        for (idx, &(_room_id, _node_id, obstacles)) in self.vertex_isv.keys.iter().enumerate() {
            if obstacles == 0 { 
                current_idx = current_idx + 1;
                regions_map_reverse.push(idx)
            }
            regions_map.push(current_idx - 1);
        }
        (regions_map, regions_map_reverse)
    }
    
    fn get_location_names(&self) -> Vec<String> {
        let mut item_loc: Vec<String> = Vec::new();
        for i in 0..self.item_locations.len() {
            item_loc.push(self.node_json_map[&self.item_locations[i]]["name"].to_string());
        }
        item_loc
    }

    fn get_location_addresses(&self) -> Vec<usize> {
        let mut addresses: Vec<usize> = Vec::new();
        for i in 0..self.item_locations.len() {
            addresses.push(self.node_ptr_map[&self.item_locations[i]]);
        }
        addresses
    }

    fn get_flag_location_names(&self) -> Vec<String> {
        let mut item_loc: Vec<String> = Vec::new();
        for i in 0..self.flag_locations.len() {
            if !item_loc.contains(&self.flag_isv.keys[self.flag_locations[i].2]) { 
                item_loc.push(self.flag_isv.keys[self.flag_locations[i].2].clone());
            }
        }
        item_loc
    }

    fn get_event_vertex_ids(&self) -> HashMap<usize, Vec<usize>> {
        let mut flag_vertex_ids: HashMap<usize, Vec<usize>> = HashMap::new();
        for &(room_id, node_id, flag_id) in &self.flag_locations {
            flag_vertex_ids.entry(self.vertex_isv.index_by_key[&(room_id, node_id, 0)]).or_insert_with(Vec::new).push(flag_id);
        }
        flag_vertex_ids
    }

    fn get_vertex_names(&self) -> Vec<(String, Option<String>)> {
        let mut nodes: Vec<(String, Option<String>)> = Vec::new();
        for &(room_id, node_id, obstacles) in &self.vertex_isv.keys {
            if obstacles == 0 { 
                let mut location_name = None;
                if self.item_locations.contains(&(room_id, node_id)) {
                    location_name = Some(self.node_json_map[&(room_id, node_id)]["name"].to_string());
                }
                /*else {
                    for i in 0..self.flag_locations.len() {
                        if self.flag_locations[i].0 == room_id && self.flag_locations[i].1 == node_id {
                            let flag_name = self.flag_isv.keys[self.flag_locations[i].2].clone();
                            location_name = Some(format!("{flag_name} ({room_id}, {node_id})"));
                            break;
                        }
                    } 
                }*/
                nodes.push((self.node_json_map[&(room_id, node_id)]["name"].to_string(), location_name));
            }
        }
        nodes
    }
}

#[pyfunction]
fn create_gamedata(apworld_path: Option<String>) -> GameData {
    let sm_json_data_path = Path::new("worlds/sm_map_rando/data/sm-json-data");
    let room_geometry_path = Path::new("worlds/sm_map_rando/data/room_geometry.json");
    //let palettes_path = Path::new("worlds/sm_map_rando/data/palettes.json");
    let escape_timings_path = Path::new("worlds/sm_map_rando/data/escape_timings.json");
    let start_locations_path = Path::new("worlds/sm_map_rando/data/start_locations.json");
    let hub_locations_path = Path::new("worlds/sm_map_rando/data/hub_locations.json");
    //let mosaic_path = Path::new("worlds/sm_map_rando/data/Mosaic");
    GameData::load(
        sm_json_data_path, 
        room_geometry_path, 
        Path::new(""),
        escape_timings_path,
        start_locations_path,
        hub_locations_path,
        Path::new(""),
        apworld_path).unwrap()
}

#[pyfunction]
fn patch_rom(
    base_rom_path: String,
    randomizer: &Randomizer,
    item_placement_ids: Vec<usize>,
    state: &RandomizationState
) -> Vec<u8> {
    let rom_path = Path::new(&base_rom_path);
    let base_rom = Rom::load(rom_path).unwrap();
    let item_placement: Vec<Item> = item_placement_ids.iter().map(|v| Item::try_from(*v).unwrap()).collect::<Vec<_>>();

    let spoiler_escape = escape_timer::compute_escape_data(&randomizer.game_data, &randomizer.map, &randomizer.difficulty_tiers[0]).unwrap();
    let spoiler_log = SpoilerLog {
        summary: Vec::new(),
        escape: spoiler_escape,
        details: Vec::new(),
        all_items: Vec::new(),
        all_rooms: Vec::new(),
    };
    let randomization = Randomization {
        difficulty: randomizer.difficulty_tiers[0].clone(),
        map: *randomizer.map.clone(),
        locked_doors: randomizer.locked_doors.to_vec(),
        item_placement: item_placement,
        spoiler_log: spoiler_log,
        seed: 0, //display_seed,
        display_seed: 0,
        start_location: state.start_location.clone(),
    };
    make_rom(&base_rom, &randomization, &randomizer.game_data).unwrap().data
}

#[pymodule]
#[pyo3(name = "pysmmaprando")]
fn pysmmaprando(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Map>()?;
    m.add_class::<GameData>()?;
    m.add_class::<DifficultyConfig>()?;
    m.add_class::<Item>()?;
    m.add_class::<APRandomizer>()?;
    m.add_class::<APCollectionState>()?;
    m.add_class::<RandomizationState>()?;    
    m.add_class::<Options>()?;
    m.add_class::<LocalState>()?;
    m.add_wrapped(wrap_pyfunction!(create_gamedata))?;
    m.add_wrapped(wrap_pyfunction!(patch_rom))?;
    Ok(())
}
