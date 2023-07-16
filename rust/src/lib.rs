pub mod game_data;
pub mod traverse;
pub mod randomize;
pub mod patch;
pub mod spoiler_map;
pub mod seed_repository;
pub mod web;
pub mod customize;

use patch::Rom;
use pyo3::{prelude::*, types::PyDict};
use randomize::{Randomization, SpoilerLog, escape_timer, ItemPlacementStyle, ItemPriorityGroup, ItemMarkers};
use crate::{
    game_data::{GameData, Map, IndexedVec, Item, NodeId, RoomId, ObstacleMask},
    randomize::{Randomizer, DifficultyConfig, VertexInfo},
    traverse::{GlobalState, LocalState, apply_requirement, compute_cost},
    patch::make_rom,
};
use std::{path::{Path, PathBuf}, mem::transmute};
use std::fs;
use reqwest::blocking::get;
use anyhow::{Context, Result};
use serde_derive::Deserialize;
use url::Url;
use hashbrown::{HashMap, HashSet};

#[derive(Deserialize, Clone)]
struct Preset {
    name: String,
    shinespark_tiles: usize,
    resource_multiplier: f32,
    escape_timer_multiplier: f32,
    phantoon_proficiency: f32,
    draygon_proficiency: f32,
    ridley_proficiency: f32,
    botwoon_proficiency: f32,
    tech: Vec<String>,
    notable_strats: Vec<String>,
}

#[derive(Clone)]
struct PresetData {
    preset: Preset,
    tech_setting: Vec<(String, bool)>,
    notable_strat_setting: Vec<(String, bool)>,
}

fn init_presets(presets: Vec<Preset>, game_data: &GameData, ignored_notable_strats: &HashSet<String>) -> Vec<PresetData> {
    let mut out: Vec<PresetData> = Vec::new();
    let mut cumulative_tech: HashSet<String> = HashSet::new();
    let mut cumulative_strats: HashSet<String> = HashSet::new();

    // Tech which is currently not used by any strat in logic, so we avoid showing on the website:
    let ignored_tech: HashSet<String> = [
        "canWallIceClip",
        "canGrappleClip",
        "canShinesparkWithReserve",
        "canRiskPermanentLossOfAccess",
        "canSpeedZebetitesSkip",
        "canRemorphZebetiteSkip",
        //"canEnterRMode",
        //"canEnterGMode",
        //"canEnterGModeImmobile",
        //"canArtificialMorph",
    ]
    .iter()
    .map(|x| x.to_string())
    .collect();
    for tech in &ignored_tech {
        if !game_data.tech_isv.index_by_key.contains_key(tech) {
            panic!("Unrecognized ignored tech \"{tech}\"");
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
        .filter(|&x| !ignored_tech.contains(x))
        .cloned()
        .collect();
    let visible_tech_set: HashSet<String> = visible_tech.iter().cloned().collect();

    let visible_notable_strats: HashSet<String> = all_notable_strats
        .iter()
        .filter(|&x| !ignored_notable_strats.contains(x))
        .cloned()
        .collect();

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
        "Frozen Geemer Alcatraz Escape",
        "Suitless Botwoon Kill",
        "Maridia Bug Room Frozen Menu Bridge",
        "Breaking the Maridia Tube Gravity Jump",
        "Crumble Shaft Consecutive Crumble Climb",
        "Metroid Room 1 PB Dodge Kill (Left to Right)",
        "Metroid Room 1 PB Dodge Kill (Right to Left)",
        "Metroid Room 2 PB Dodge Kill (Bottom to Top)",
        "Metroid Room 3 PB Dodge Kill (Left to Right)",
        "Metroid Room 3 PB Dodge Kill (Right to Left)",
        "Metroid Room 4 Three PB Kill (Top to Bottom)",
        "Metroid Room 4 Six PB Dodge Kill (Bottom to Top)",
        "Metroid Room 4 Three PB Dodge Kill (Bottom to Top)",
        "Partial Covern Ice Clip",
        "Basement Speedball (Phantoon Dead)",
        "Basement Speedball (Phantoon Alive)",
        "MickeyMouse Crumbleless MidAir Spring Ball",
        "Mickey Mouse Crumble IBJ",
        "Botwoon Hallway Puyo Ice Clip",
    ]
    .iter()
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
    save_animals: bool,
    #[pyo3(get, set)]
    objectives: u8,
    #[pyo3(get, set)]
    filler_items: String,
    #[pyo3(get, set)]
    quality_of_life_preset: usize,
    #[pyo3(get, set)]
    supers_double: bool,
    #[pyo3(get, set)]
    mother_brain_short: bool,
    #[pyo3(get, set)]
    escape_enemies_cleared: bool,
    #[pyo3(get, set)]
    escape_movement_items: bool,
    #[pyo3(get, set)]
    mark_map_stations: bool,
    #[pyo3(get, set)]
    item_markers: u8,
    #[pyo3(get, set)]
    all_items_spawn: bool,
    #[pyo3(get, set)]
    fast_elevators: bool,
    #[pyo3(get, set)]
    fast_doors: bool,
    #[pyo3(get, set)]
    vanilla_map: bool, 
}

#[pymethods]
impl Options{
    #[new]
    pub fn new( preset: usize,
                techs: Vec<String>,
                strats: Vec<String>,
                shinespark_tiles: usize,
                resource_multiplier: f32,
                phantoon_proficiency: f32,
                draygon_proficiency: f32,
                ridley_proficiency: f32,
                botwoon_proficiency: f32,
                escape_timer_multiplier: f32,
                save_animals: bool,
                objectives: u8,
                filler_items: String,
                quality_of_life_preset: usize,
                supers_double: bool,
                mother_brain_short: bool,
                escape_enemies_cleared: bool,
                escape_movement_items: bool,
                mark_map_stations: bool,
                item_markers: u8,
                all_items_spawn: bool,
                fast_elevators: bool,
                fast_doors: bool,
                vanilla_map: bool) -> Self {
        Options { 
            preset,
            techs,
            strats,
            shinespark_tiles,
            resource_multiplier,
            phantoon_proficiency,
            draygon_proficiency,
            ridley_proficiency,
            botwoon_proficiency,
            escape_timer_multiplier,
            save_animals,
            objectives,
            filler_items,
            quality_of_life_preset,
            supers_double,
            mother_brain_short,
            escape_enemies_cleared,
            escape_movement_items,
            mark_map_stations,
            item_markers,
            all_items_spawn,
            fast_elevators,
            fast_doors,
            vanilla_map
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


#[pyclass]
#[derive(Clone)]
pub struct APCollectionState {
    #[pyo3(get)]
    global_state: GlobalState,
    local_states: Vec<Option<LocalState>>,
    #[pyo3(get)]
    cost: Vec<f32>,
    ap_randomizer: Option<Box<APRandomizer>>,
}

#[pymethods]
impl APCollectionState{
    #[new]
    pub fn new(
        #[pyo3(from_py_with = "make_optional_box")]
        ap_randomizer: Option<Box<APRandomizer>>) -> Self {
        let global_state = match &ap_randomizer {
            Some(ap_r) => {
                    let rando = &ap_r.randomizer;
                    let items = vec![false; rando.game_data.item_isv.keys.len() - 1];
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
        let num_vertices = ap_randomizer.as_ref().unwrap().randomizer.game_data.vertex_isv.keys.len();
        let local_states = vec![Some(LocalState {
                energy_used: 0,
                reserves_used: 0,
                missiles_used: 0,
                supers_used: 0,
                power_bombs_used: 0
            }); num_vertices];
        let cost = vec![f32::INFINITY; num_vertices];
        APCollectionState { 
            global_state,
            local_states,
            cost,
            ap_randomizer,
        }
    }

    fn can_traverse(&mut self, ap_region_from_id: usize, strats_links: HashMap<String, Vec<usize>>) -> bool {
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
    }

    pub fn add_item(&mut self, item: usize) {
        self.global_state.collect(unsafe { transmute(item) }, &self.ap_randomizer.as_ref().unwrap().randomizer.game_data);
    }

    pub fn remove_item(&mut self, item: usize) {
        self.global_state.remove(unsafe { transmute(item) }, &self.ap_randomizer.as_ref().unwrap().randomizer.game_data);
    }

    pub fn add_flag(&mut self, flag: usize) {
        self.global_state.flags[flag] = true;
    }

    pub fn remove_flag(&mut self, flag: usize) {
        self.global_state.flags[flag] = false;
    }

    pub fn __deepcopy__(&self, _memo: &PyDict) -> Self {self.clone()}
}

fn get_difficulty_config(options: &Options, preset_data: &Vec<PresetData>, game_data: &GameData) -> DifficultyConfig {
    let preset: Preset = match options.preset {
        index if options.preset < preset_data.len() => preset_data[index].preset.clone(),
        _ => Preset {
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
     };
    DifficultyConfig {
        tech: preset.tech,
        notable_strats: preset.notable_strats,
        shine_charge_tiles: preset.shinespark_tiles as f32,
        progression_rate: randomize::ProgressionRate::Normal,
        filler_items: vec![Item::Missile],
        item_placement_style: ItemPlacementStyle::Neutral,
        item_priorities: vec![ItemPriorityGroup {
            name: "Default".to_string(),
            items: game_data.item_isv.keys.clone(),
        }],
        resource_multiplier: preset.resource_multiplier,
        escape_timer_multiplier: preset.escape_timer_multiplier,
        save_animals: options.save_animals,
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
        mother_brain_short: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.mother_brain_short,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        escape_enemies_cleared: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.escape_enemies_cleared,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
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
        item_markers: match options.quality_of_life_preset {
            0 => ItemMarkers::Basic,
            1 => ItemMarkers::ThreeTiered,
            2 => unsafe { transmute(options.item_markers) },
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
        all_items_spawn: match options.quality_of_life_preset {
            0 => false,
            1 => true,
            2 => options.all_items_spawn,
            _ => panic!("Unrecognized quality_of_life_preset: {}", options.quality_of_life_preset)
        },
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
        objectives: unsafe { transmute(options.objectives) },
        vanilla_map: options.vanilla_map,
        debug_options: None,
    }
}

#[pyclass]
#[derive(Clone)]
pub struct APRandomizer {
    #[pyo3(get)]
    randomizer: Randomizer,
    regions_map: Vec<usize>,
    regions_map_reverse: Vec<usize>, 
    preset_datas: Vec<PresetData>,
}

#[pymethods]
impl APRandomizer{
    #[new]
    pub fn new(options: Options, map_seed: usize) -> Self {
        let sm_json_data_path = Path::new("worlds/sm_map_rando/data/sm-json-data");
        let room_geometry_path = Path::new("worlds/sm_map_rando/data/room_geometry.json");
        let palettes_path = Path::new("worlds/sm_map_rando/data/palettes.json");
        let game_data: GameData = GameData::load(sm_json_data_path, room_geometry_path, palettes_path).unwrap();

        let presets: Vec<Preset> = serde_json::from_str(&std::fs::read_to_string(&"worlds/sm_map_rando/data/presets.json").unwrap()).unwrap();
        let ignored_notable_strats = get_ignored_notable_strats();
        let preset_datas = init_presets(presets, &game_data, &ignored_notable_strats);
        let difficulty_tiers = vec![get_difficulty_config(&options, &preset_datas, &game_data); 1];

        let binding = get_map_repository("worlds/sm_map_rando/data/mapRepository.json").unwrap();
        let map_repository_array = binding.as_slice();
        let map = if difficulty_tiers[0].vanilla_map {
            get_vanilla_map().unwrap()
        } else {
            get_map(Path::new("https://storage.googleapis.com/super-metroid-map-rando/maps/session-2022-06-03T17%3A19%3A29.727911.pkl-bk30-subarea-balance-2/"),
                            map_repository_array,
                            TryInto::<usize>::try_into(map_seed).unwrap()).unwrap()
        };

        let randomizer = Randomizer::new(Box::new(map), Box::new(difficulty_tiers), Box::new(game_data));

        let (regions_map, regions_map_reverse) = randomizer.game_data.get_regions_map();

        APRandomizer { 
            randomizer,
            regions_map,
            regions_map_reverse,
            preset_datas,
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
}

#[derive(Deserialize)]
pub struct MapRepository {
    pub map_array: Vec<String>,
}

fn get_map_repository(path: &str) -> Result<Vec<String>> {
    let contents = fs::read_to_string(path)?;
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

fn get_vanilla_map() -> Result<Map> {
    let path = Path::new("worlds/sm_map_rando/data/vanilla_map.json");
    let map_string = std::fs::read_to_string(&path)
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

    fn get_event_location_names(&self) -> Vec<String> {
        let mut flag_loc: Vec<String> = Vec::new();
        for &(room_id, node_id, flag_id) in &self.flag_locations {
            let flag_name = self.flag_isv.keys[flag_id].clone();
            // println!("{} {} {}", room_id, node_id, flag_name);
            flag_loc.push(format!("{flag_name} ({room_id}, {node_id})"));
        }
        flag_loc
    }

    fn get_vertex_names(&self) -> Vec<(String, Option<String>)> {
        let mut nodes: Vec<(String, Option<String>)> = Vec::new();
        for &(room_id, node_id, obstacles) in &self.vertex_isv.keys {
            if obstacles == 0 { 
                let mut location_name = None;
                if self.item_locations.contains(&(room_id, node_id)) {
                    location_name = Some(self.node_json_map[&(room_id, node_id)]["name"].to_string());
                }
                else {
                    for i in 0..self.flag_locations.len() {
                        if self.flag_locations[i].0 == room_id && self.flag_locations[i].1 == node_id {
                            let flag_name = self.flag_isv.keys[self.flag_locations[i].2].clone();
                            location_name = Some(format!("{flag_name} ({room_id}, {node_id})"));
                            break;
                        }
                    } 
                }
                nodes.push((self.node_json_map[&(room_id, node_id)]["name"].to_string(), location_name));
            }
        }
        nodes
    }
}

#[pyfunction]
fn create_gamedata() -> GameData {
    let sm_json_data_path = Path::new("worlds/sm_map_rando/data/sm-json-data");
    let room_geometry_path = Path::new("worlds/sm_map_rando/data/room_geometry.json");
    let palettes_path = Path::new("worlds/sm_map_rando/data/palettes.json");
    GameData::load(sm_json_data_path, room_geometry_path, palettes_path).unwrap()
}

#[pyfunction]
fn patch_rom(
    base_rom_path: String,
    randomizer: &Randomizer,
    item_placement_ids: Vec<usize>,
) -> Vec<u8> {
    let rom_path = Path::new(&base_rom_path);
    let base_rom = Rom::load(rom_path).unwrap();
    println!("{:?}", base_rom_path);
    println!("{:?}", randomizer.difficulty_tiers);
    println!("{:?}", item_placement_ids);

    let item_placement: Vec<Item> = item_placement_ids.iter().map(|v| Item::try_from(*v).unwrap()).collect::<Vec<_>>();
    println!("{:?}", item_placement);

    let spoiler_escape = escape_timer::compute_escape_data(&randomizer.game_data, &randomizer.map, &randomizer.difficulty_tiers[0]);
    let spoiler_log = SpoilerLog {
        summary: Vec::new(),
        escape: spoiler_escape,
        details: Vec::new(),
        all_items: Vec::new(),
        all_rooms: Vec::new(),
    };
    println!("SpoilerLog created");
    let randomization = Randomization {
        difficulty: randomizer.difficulty_tiers[0].clone(),
        map: *randomizer.map.clone(),
        item_placement: item_placement,
        spoiler_log: spoiler_log,
    };
    println!("Randomization created");
    make_rom(&base_rom, &randomization, &randomizer.game_data).unwrap().data
}

#[pymodule]
#[pyo3(name = "map_randomizer")]
fn map_randomizer(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Map>()?;
    m.add_class::<GameData>()?;
    m.add_class::<DifficultyConfig>()?;
    m.add_class::<Item>()?;
    m.add_class::<APRandomizer>()?;
    m.add_class::<APCollectionState>()?;
    m.add_class::<Options>()?;
    m.add_wrapped(wrap_pyfunction!(create_gamedata))?;
    m.add_wrapped(wrap_pyfunction!(patch_rom))?;
    Ok(())
}
