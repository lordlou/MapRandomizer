pub mod game_data;
pub mod traverse;
pub mod randomize;
pub mod patch;
pub mod spoiler_map;
pub mod seed_repository;
pub mod web;
pub mod customize;

use customize::{customize_rom, ControllerConfig, CustomizeSettings, MusicSettings};
use game_data::{StartLocation, HubLocation, LinksDataGroup};
use patch::{ips_write::create_ips_patch, Rom};
use pyo3::{prelude::*, types::PyDict};
use rand::{SeedableRng, RngCore};
use randomize::{Randomization, SpoilerLog, escape_timer, randomize_doors, ItemPlacementStyle, ItemPriorityGroup, ItemMarkers, RandomizationState, ItemLocationState, FlagLocationState, SaveLocationState, MotherBrainFight, SpoilerSummary, SpoilerItemSummary, SpoilerLocation, SpoilerFlagSummary};
use traverse::TraverseResult;
use crate::{
    game_data::{GameData, IndexedVec, Item, Map, NodeId, ObstacleMask, RoomId}, patch::make_rom, randomize::{DifficultyConfig, Randomizer, SaveAnimals, StartLocationMode, VertexInfo}, traverse::{get_bireachable_idxs, traverse, GlobalState, LocalState}
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
    heated_shinespark_tiles: usize,
    shinecharge_leniency_frames: i32,
    resource_multiplier: f32,
    escape_timer_multiplier: f32,
    gate_glitch_leniency: i32,
    door_stuck_leniency: i32,
    phantoon_proficiency: f32,
    draygon_proficiency: f32,
    ridley_proficiency: f32,
    botwoon_proficiency: f32,
    mother_brain_proficiency: f32,
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
    implicit_tech: &HashSet<String>,
) -> Vec<PresetData> {
    let mut out: Vec<PresetData> = Vec::new();
    let mut cumulative_tech: HashSet<String> = HashSet::new();
    let mut cumulative_strats: HashSet<String> = HashSet::new();

    // Tech which is currently not used by any strat in logic, so we avoid showing on the website:
    let ignored_tech: HashSet<String> = [
        "canRiskPermanentLossOfAccess",
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
        .all_links()
        .filter_map(|x| x.notable_strat_name.clone())
        .collect();

    let visible_tech: Vec<String> = game_data
        .tech_isv
        .keys
        .iter()
        .filter(|&x| !ignored_tech.contains(x) && !implicit_tech.contains(x))
        .cloned()
        .collect();
    let visible_tech_set: HashSet<String> = visible_tech.iter().cloned().collect();

    // TODO: remove this
    let visible_notable_strats: HashSet<String> = all_notable_strats
        .iter()
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

fn get_implicit_tech() -> HashSet<String> {
    [
        "canSpecialBeamAttack",
        "canMidAirMorph",
        "canTurnaroundSpinJump",
        "canStopOnADime",
        "canUseGrapple",
        "canEscapeEnemyGrab",
        "canDownBack",
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
    heated_shinespark_tiles: usize,
    #[pyo3(get, set)]
    shinecharge_leniency_frames: i32,
    #[pyo3(get, set)]
    resource_multiplier: f32,
    #[pyo3(get, set)]
    gate_glitch_leniency: i32,
    #[pyo3(get, set)]
    door_stuck_leniency: i32,
    #[pyo3(get, set)]
    phantoon_proficiency: f32,
    #[pyo3(get, set)]
    draygon_proficiency: f32,
    #[pyo3(get, set)]
    ridley_proficiency: f32,
    #[pyo3(get, set)]
    botwoon_proficiency: f32,
    #[pyo3(get, set)]
    mother_brain_proficiency: f32,
    #[pyo3(get, set)]
    escape_timer_multiplier: f32,
    #[pyo3(get, set)]
    start_location_mode: u8,
    #[pyo3(get, set)]
    save_animals: u8,
    #[pyo3(get, set)]
    early_save: bool,
    #[pyo3(get, set)]
    objectives: u8,
    #[pyo3(get, set)]
    doors_mode: u8,
    #[pyo3(get, set)]
    area_assignment: bool,
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
    room_outline_revealed: bool,
    #[pyo3(get, set)]
    transition_letters: bool, 
    #[pyo3(get, set)]
    item_markers: u8,
    #[pyo3(get, set)]
    item_dots_disappear: bool,
    #[pyo3(get, set)]
    all_items_spawn: bool,
    #[pyo3(get, set)]
    buffed_drops: bool,
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
    wall_jump: usize,
    #[pyo3(get, set)]
    etank_refill: usize,
    #[pyo3(get, set)]
    maps_revealed: u8,
    #[pyo3(get, set)]
    map_layout: usize,
    #[pyo3(get, set)]
    energy_free_shinesparks: bool,
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
                heated_shinespark_tiles: usize,
                shinecharge_leniency_frames: i32,
                resource_multiplier: f32,
                gate_glitch_leniency: i32,
                door_stuck_leniency: i32,
                phantoon_proficiency: f32,
                draygon_proficiency: f32,
                ridley_proficiency: f32,
                botwoon_proficiency: f32,
                mother_brain_proficiency: f32,
                escape_timer_multiplier: f32,
                start_location_mode: u8,
                save_animals: u8,
                early_save: bool,
                objectives: u8,
                doors_mode: u8,
                area_assignment: bool,
                filler_items: String,
                supers_double: bool,
                mother_brain_fight: u8,
                escape_enemies_cleared: bool,
                escape_refill: bool,
                escape_movement_items: bool,
                mark_map_stations: bool,
                room_outline_revealed: bool,
                transition_letters: bool,
                item_markers: u8,
                item_dots_disappear: bool,
                all_items_spawn: bool,
                buffed_drops: bool,
                acid_chozo: bool,
                fast_elevators: bool,
                fast_doors: bool,
                fast_pause_menu: bool,
                respin: bool,
                infinite_space_jump: bool,
                momentum_conservation: bool,
                wall_jump: usize,
                etank_refill: usize,
                maps_revealed: u8,
                map_layout: usize,
                energy_free_shinesparks: bool,
                ultra_low_qol: bool,
                skill_assumptions_preset: String,
                item_progression_preset: String,
                quality_of_life_preset: usize) -> Self {
        Options { 
            preset,
            techs,
            strats,
            shinespark_tiles,
            heated_shinespark_tiles,
            shinecharge_leniency_frames,
            resource_multiplier,
            gate_glitch_leniency,
            door_stuck_leniency,
            phantoon_proficiency,
            draygon_proficiency,
            ridley_proficiency,
            botwoon_proficiency,
            mother_brain_proficiency,
            escape_timer_multiplier,
            start_location_mode,
            save_animals,
            early_save,
            objectives,
            doors_mode,
            area_assignment,
            filler_items,
            supers_double,
            mother_brain_fight,
            escape_enemies_cleared,
            escape_refill,
            escape_movement_items,
            mark_map_stations,
            room_outline_revealed,
            transition_letters,
            item_markers,
            item_dots_disappear,
            all_items_spawn,
            buffed_drops,
            acid_chozo,
            fast_elevators,
            fast_doors,
            fast_pause_menu,
            respin,
            infinite_space_jump,
            momentum_conservation,
            wall_jump,
            etank_refill,
            maps_revealed,
            map_layout,
            energy_free_shinesparks,
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
            difficulty_tier: None,
        };
        let initial_flag_location_state = FlagLocationState {
            reachable: false,
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
                        heated_shine_charge_tiles: rando.difficulty_tiers[0].heated_shine_charge_tiles,
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
                heated_shine_charge_tiles: 0.0,
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
            randomization_state
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

    pub fn add_item(&mut self, item: usize, game_data: &GameData) {
        self.randomization_state.global_state.collect(unsafe { transmute(item) }, game_data);
    }

    pub fn remove_item(&mut self, item: usize, game_data: &GameData) {
        self.randomization_state.global_state.remove(unsafe { transmute(item) }, game_data);
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
            heated_shinespark_tiles: pd.preset.heated_shinespark_tiles,
            resource_multiplier: pd.preset.resource_multiplier,
            escape_timer_multiplier: pd.preset.escape_timer_multiplier,
            shinecharge_leniency_frames: pd.preset.shinecharge_leniency_frames,
            gate_glitch_leniency: pd.preset.gate_glitch_leniency,
            door_stuck_leniency: pd.preset.door_stuck_leniency,
            phantoon_proficiency: pd.preset.phantoon_proficiency,
            draygon_proficiency: pd.preset.draygon_proficiency,
            ridley_proficiency: pd.preset.ridley_proficiency,
            botwoon_proficiency: pd.preset.botwoon_proficiency,
            mother_brain_proficiency: pd.preset.mother_brain_proficiency,
            tech: tech_vec,
            notable_strats: strat_vec,
        }
    }
    else {
        preset = Preset {
            name: "Custom".to_string(),
            shinespark_tiles: options.shinespark_tiles,
            heated_shinespark_tiles: options.heated_shinespark_tiles,
            resource_multiplier: options.resource_multiplier,
            escape_timer_multiplier: options.escape_timer_multiplier,
            shinecharge_leniency_frames: options.shinecharge_leniency_frames,
            gate_glitch_leniency: options.gate_glitch_leniency,
            door_stuck_leniency: options.door_stuck_leniency,
            phantoon_proficiency: options.phantoon_proficiency,
            draygon_proficiency: options.draygon_proficiency,
            ridley_proficiency: options.ridley_proficiency,
            botwoon_proficiency: options.botwoon_proficiency,
            mother_brain_proficiency: options.mother_brain_proficiency,
            tech: options.techs.clone(),
            notable_strats: options.strats.clone(),
            }
    }

    let qol_preset;
    if options.ultra_low_qol {
        qol_preset = 0;
    }
    else {
        qol_preset = options.quality_of_life_preset;
    }

    DifficultyConfig {
        name: Some(preset.name),
        tech: preset.tech,
        notable_strats: preset.notable_strats,
        shine_charge_tiles: preset.shinespark_tiles as f32,
        heated_shine_charge_tiles: preset.heated_shinespark_tiles as f32,
        shinecharge_leniency_frames:  preset.shinecharge_leniency_frames,
        progression_rate: randomize::ProgressionRate::Uniform,
        random_tank: true,
        spazer_before_plasma: false,
        stop_item_placement_early: false,
        item_pool: vec![],
        starting_items: vec![],
        semi_filler_items: vec![],
        filler_items: vec![Item::Missile],
        early_filler_items: vec![],
        item_placement_style: ItemPlacementStyle::Neutral,
        item_priorities: vec![ItemPriorityGroup {
            name: "Default".to_string(),
            items: game_data.item_isv.keys.clone(),
        }],
        resource_multiplier: preset.resource_multiplier,
        gate_glitch_leniency: preset.gate_glitch_leniency,
        door_stuck_leniency: preset.door_stuck_leniency,
        escape_timer_multiplier: preset.escape_timer_multiplier,
        start_location_mode: match options.start_location_mode {
            0 => StartLocationMode::Ship,
            1 => StartLocationMode::Random,
            2 => StartLocationMode::Escape,
            _ => panic!("Unrecognized start_location_mode: {}", options.start_location_mode)
        },
        save_animals: match options.save_animals {
            0 => SaveAnimals::No,
            1 => SaveAnimals::Maybe,
            2 => SaveAnimals::Yes,
            _ => panic!("Unrecognized save_animals: {}", options.save_animals)
        },
        early_save: options.early_save,
        phantoon_proficiency: preset.phantoon_proficiency,
        draygon_proficiency: preset.draygon_proficiency,
        ridley_proficiency: preset.ridley_proficiency,
        botwoon_proficiency: preset.botwoon_proficiency,
        mother_brain_proficiency: preset.mother_brain_proficiency,
        supers_double: match qol_preset {
            0 => false,
            1 => true,
            2 => true,
            3 => true,
            4 => options.supers_double,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        mother_brain_fight: match qol_preset {
            0 => MotherBrainFight::Vanilla,
            1 => MotherBrainFight::Short,
            2 => MotherBrainFight::Short,
            3 => MotherBrainFight::Skip,
            4 => unsafe { transmute(options.mother_brain_fight)},
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        escape_enemies_cleared: match qol_preset {
            0 => false,
            1 => false,
            2 => true,
            3 => true,
            4 => options.escape_enemies_cleared,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        escape_refill: match qol_preset {
            0 => false,
            1 => false,
            2 => true,
            3 => true,
            4 => options.escape_refill,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        escape_movement_items: match qol_preset {
            0 => false,
            1 => false,
            2 => true,
            3 => true,
            4 => options.escape_movement_items,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        mark_map_stations: match qol_preset {
            0 => false,
            1 => true,
            2 => true,
            3 => true,
            4 => options.mark_map_stations,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        room_outline_revealed: options.room_outline_revealed,
        transition_letters: options.transition_letters,
        item_markers: match qol_preset {
            0 => ItemMarkers::Simple,
            1 => ItemMarkers::Uniques,
            2 => ItemMarkers::ThreeTiered,
            3 => ItemMarkers::ThreeTiered,
            4 => unsafe { transmute(options.item_markers) },
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        item_dot_change: match options.item_dots_disappear {
            false => randomize::ItemDotChange::Fade,
            true => randomize::ItemDotChange::Disappear,
        },
        all_items_spawn: match qol_preset {
            0 => false,
            1 => false,
            2 => true,
            3 => true,
            4 => options.all_items_spawn,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        buffed_drops: match qol_preset {
            0 => false,
            1 => false,
            2 => true,
            3 => true,
            4 => options.buffed_drops,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        acid_chozo: match qol_preset {
            0 => false,
            1 => false,
            2 => true,
            3 => true,
            4 => options.acid_chozo,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        fast_elevators: match qol_preset {
            0 => false,
            1 => true,
            2 => true,
            3 => true,
            4 => options.fast_elevators,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        fast_doors: match qol_preset {
            0 => false,
            1 => true,
            2 => true,
            3 => true,
            4 => options.fast_doors,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        fast_pause_menu: match qol_preset {
            0 => false,
            1 => true,
            2 => true,
            3 => true,
            4 => options.fast_pause_menu,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        respin: match qol_preset {
            0 => false,
            1 => false,
            2 => false,
            3 => true,
            4 => options.respin,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        infinite_space_jump: match qol_preset {
            0 => false,
            1 => false,
            2 => false,
            3 => true,
            4 => options.infinite_space_jump,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        momentum_conservation: match qol_preset {
            0 => false,
            1 => false,
            2 => false,
            3 => true,
            4 => options.momentum_conservation,
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
        objectives: match options.objectives {
            0 => randomize::Objectives::None,
            1 => randomize::Objectives::Bosses,
            2 => randomize::Objectives::Minibosses,
            3 => randomize::Objectives::Metroids,
            4 => randomize::Objectives::Chozos,
            5 => randomize::Objectives::Pirates,
            _ => panic!("Unrecognized objectives: {}", options.objectives)
        },
        doors_mode: match options.doors_mode {
            0 => randomize::DoorsMode::Blue,
            1 => randomize::DoorsMode::Ammo,
            _ => panic!("Unrecognized doors_mode: {}", options.doors_mode)
        },
        area_assignment: match options.area_assignment {
            false => randomize::AreaAssignment::Standard,
            true => randomize::AreaAssignment::Random,
        },
        wall_jump: match options.wall_jump {
            0 => randomize::WallJump::Vanilla,
            1 => randomize::WallJump::Collectible,
            _ => panic!("Unrecognized doors_mode: {}", options.doors_mode)
        },
        etank_refill: match options.etank_refill {
            0 => randomize::EtankRefill::Disabled,
            1 => randomize::EtankRefill::Vanilla,
            2 => randomize::EtankRefill::Full,
            _ => panic!("Unrecognized doors_mode: {}", options.doors_mode)
        },
        maps_revealed: match options.maps_revealed {
            0 => randomize::MapsRevealed::No,
            1 => randomize::MapsRevealed::Partial,
            2 => randomize::MapsRevealed::Yes,
            _ => panic!("Unrecognized maps_revealed: {}", options.maps_revealed)
        },
        map_layout: options.map_layout,
        vanilla_map: options.map_layout == 0,
        energy_free_shinesparks: options.energy_free_shinesparks,
        ultra_low_qol: options.ultra_low_qol,
        skill_assumptions_preset: Some("".to_string()/*options.skill_assumptions_preset*/),
        item_progression_preset: Some("".to_string()/*options.item_progression_preset*/),
        quality_of_life_preset: match qol_preset {
            0 => Some("Off".to_string()),
            1 => Some("Low".to_string()),
            2 => Some("Default".to_string()),
            3 => Some("Max".to_string()),
            4 => Some("Custom".to_string()),
            _ => panic!("Unrecognized quality_of_life_preset: {}", qol_preset)
        },
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
        let implicit_tech = get_implicit_tech();
        let preset_datas = init_presets(presets, game_data, &implicit_tech);
        let difficulty_tiers = vec![get_difficulty_config(&options, &preset_datas, game_data); 1];

        let (map_repo_filename, map_repo_url) = if difficulty_tiers[0].map_layout == 1 { 
            ("worlds/sm_map_rando/data/mapRepositoryTame.json",
            "https://storage.googleapis.com/super-metroid-map-rando/maps/v110c-tame/")
        }
        else {
            ("worlds/sm_map_rando/data/mapRepositoryWild.json",
            "https://storage.googleapis.com/super-metroid-map-rando/maps/v110c-wild/")
        };

        let binding = get_map_repository(game_data, map_repo_filename).unwrap();
        let map_repository_array = binding.as_slice();

        let mut map = if difficulty_tiers[0].map_layout == 0 {
            get_vanilla_map(game_data).unwrap()
        } else {   
            get_map(Path::new(map_repo_url), map_repository_array, TryInto::<usize>::try_into(seed).unwrap()).unwrap()
        };
        let diff_settings = difficulty_tiers[0].clone();
        let mut locked_doors = randomize_doors(&game_data, &map, &diff_settings, seed);

        let mut randomizer = Randomizer::new(Box::new(map), Box::new(locked_doors.clone()), Box::new(difficulty_tiers.clone()), Box::new(game_data.clone()), Box::new(game_data.base_links_data.clone()), Box::new(game_data.seed_links.clone()));
        
        if diff_settings.map_layout != 0 && diff_settings.start_location_mode != StartLocationMode::Ship {
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
                    heated_shine_charge_tiles: randomizer.difficulty_tiers[0].heated_shine_charge_tiles,
                };
                let forward = traverse(
                    &randomizer.base_links_data,
                    &randomizer.seed_links_data,
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
                    &randomizer.base_links_data,
                    &randomizer.seed_links_data,
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
                        if get_bireachable_idxs(
                                &global,
                                v,
                                &forward,
                                &reverse).is_some() {
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
                    locked_doors = randomize_doors(&game_data, &map, &diff_settings, seed);
                    randomizer = Randomizer::new(Box::new(map), Box::new(locked_doors.clone()), Box::new(difficulty_tiers.clone()), Box::new(game_data.clone()), Box::new(game_data.base_links_data.clone()), Box::new(game_data.seed_links.clone()));
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
        for (idx, link) in self.randomizer.base_links_data.links.iter().chain(self.randomizer.seed_links_data.links.iter()).enumerate() {
            let key= (self.regions_map[link.from_vertex_id], self.regions_map[link.to_vertex_id]);
            links.entry(key).or_insert_with(HashMap::new).entry(link.strat_name.clone()).or_insert_with(Vec::new).push(idx);
        }
        links
    }

    pub fn get_link_requirement(&self, link_id: usize) -> String {
        format!("from:{} to:{} using {}: {:?}", 
        self.regions_map[self.randomizer.get_link(link_id).from_vertex_id], 
        self.regions_map[self.randomizer.get_link(link_id).to_vertex_id], 
            self.randomizer.get_link(link_id).strat_name, 
            self.randomizer.get_link(link_id).requirement)
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
            &self.randomizer.base_links_data,
            &self.randomizer.seed_links_data,
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
            &self.randomizer.base_links_data,
            &self.randomizer.seed_links_data,
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
            bi_reachability[i] = get_bireachable_idxs(
                                            &state.global_state,
                                            i,
                                            &forward,
                                            &reverse,
                                            ).is_some();
            f_reachability[i] = forward.local_states[i][0].is_some() && forward.local_states[i][1].is_some();
            r_reachability[i] = reverse.local_states[i][0].is_some() && reverse.local_states[i][1].is_some();

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

impl IntoPy<PyObject> for Box<LinksDataGroup> {
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
            let room_name = self.room_json_map[&self.item_locations[i].0]["name"].to_string();
            let location_name = self.node_json_map[&self.item_locations[i]]["name"].to_string();
            item_loc.push(format!("{room_name} {location_name}"));
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
                let mut complete_location_name = None;
                if self.item_locations.contains(&(room_id, node_id)) {
                    let room_name = self.room_json_map[&room_id]["name"].to_string();
                    let location_name = self.node_json_map[&(room_id, node_id)]["name"].to_string();
                    complete_location_name = Some(format!("{room_name} {location_name}"));
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
                let room_name = self.room_json_map[&room_id]["name"].to_string();
                let node_name = self.node_json_map[&(room_id, node_id)]["name"].to_string();
                nodes.push((format!("{room_name} {node_name}"), complete_location_name));
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
    let title_screen_path = Path::new("worlds/sm_map_rando/data/TitleScreen/Images");

    GameData::load(
        sm_json_data_path, 
        room_geometry_path, 
        Path::new(""),
        escape_timings_path,
        start_locations_path,
        hub_locations_path,
        Path::new(""),
        title_screen_path,
        apworld_path).unwrap()
}

fn create_summary(spoiler_summary_vec: Vec<(usize, String, String)>) -> Vec<SpoilerSummary> {
    let mut result: Vec<SpoilerSummary> = Vec::new();
    let mut current_step = 0;
    for (step, item, location) in spoiler_summary_vec {
        if step == current_step - 1 {
            result[step].items.push(SpoilerItemSummary {
                item: item,
                location: SpoilerLocation {
                    area: location,
                    room: "".to_string(),
                    node: "".to_string(),
                    coords: (0, 0),
                    },
                });
        } else {
            current_step += 1;
            result.push(SpoilerSummary {
                step: current_step,
                items: vec![SpoilerItemSummary {
                    item: item,
                    location: SpoilerLocation {
                        area: location,
                        room: "".to_string(),
                        node: "".to_string(),
                        coords: (0, 0),
                    },
                }],
                flags: vec![SpoilerFlagSummary {
                    flag: "".to_string(),
                }],
            })
        }
    }
    result
}

#[pyfunction]
fn patch_rom(
    base_rom_path: String,
    ap_randomizer: &APRandomizer,
    item_placement_ids: Vec<usize>,
    state: &RandomizationState,
    spoiler_summary_vec: Vec<(usize, String, String)>
) -> Vec<u8> {
    let rom_path = Path::new(&base_rom_path);
    let base_rom = Rom::load(rom_path).unwrap();
    let item_placement: Vec<Item> = item_placement_ids.iter().map(|v| Item::try_from(*v).unwrap()).collect::<Vec<_>>();
    let randomizer = &ap_randomizer.randomizer;

    let spoiler_escape = escape_timer::compute_escape_data(&randomizer.game_data, &randomizer.map, &randomizer.difficulty_tiers[0]).unwrap();
    let summary = create_summary(spoiler_summary_vec);
    let spoiler_log = SpoilerLog {
        summary: summary,
        escape: spoiler_escape,
        details: Vec::new(),
        all_items: Vec::new(),
        all_rooms: Vec::new(),
    };
    let randomization = Randomization {
        difficulty: randomizer.difficulty_tiers[0].clone(),
        map: *randomizer.map.clone(),
        toilet_intersections: (*randomizer.toilet_intersections.clone()).to_vec(),
        locked_doors: randomizer.locked_doors.to_vec(),
        item_placement: item_placement,
        spoiler_log: spoiler_log,
        seed: ap_randomizer.seed,
        display_seed: ap_randomizer.seed,
        start_location: state.start_location.clone(),
        starting_items: randomizer.difficulty_tiers[0].starting_items.clone(),
    };
    let game_rom = make_rom(&base_rom, &randomization, &randomizer.game_data).unwrap();
    let ips_patch = create_ips_patch(&base_rom.data, &game_rom.data);

    let mut output_rom = base_rom.clone();
    let customize_settings = CustomizeSettings {
        samus_sprite: None,
        etank_color: None,
        reserve_hud_style: true,
        vanilla_screw_attack_animation: true,
        palette_theme: customize::PaletteTheme::AreaThemed,
        tile_theme: customize::TileTheme::Constant("WreckedShip".to_string()),
        music: MusicSettings::AreaThemed,
        // music: MusicSettings::Vanilla,
        disable_beeping: false,
        shaking: customize::ShakingSetting::Disabled,
        controller_config: ControllerConfig::default(),
    };
    customize_rom(
        &mut output_rom,
        &base_rom,
        &ips_patch,
        &customize_settings,
        &randomizer.game_data,
        &vec![],
    ).unwrap();
    output_rom.data
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
