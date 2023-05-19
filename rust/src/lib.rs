pub mod game_data;
pub mod traverse;
pub mod randomize;
pub mod patch;
pub mod spoiler_map;
pub mod seed_repository;
pub mod web;
pub mod customize;

use pyo3::{prelude::*, types::PyDict};
use crate::{
    game_data::{GameData, Map, IndexedVec, Item, NodeId, RoomId, ObstacleMask},
    randomize::{Randomizer, get_difficulty_config, DifficultyConfig, VertexInfo},
    traverse::{GlobalState, LocalState, apply_requirement, compute_cost}
};
use std::{path::{Path, PathBuf}, mem::transmute};
use std::fs;
use reqwest::blocking::{get};
use anyhow::{Context, Result};
use serde_derive::Deserialize;
use url::Url;
use std::collections::HashMap;

/*
#[pyclass]
struct PyRandomizeRequest {
    rom: Py<PyBytes>,
    preset: Option<Py<PyString>>,
    shinespark_tiles: Py<PyFloat>,
    resource_multiplier: Py<PyFloat>,
    phantoon_proficiency: Py<PyFloat>,
    draygon_proficiency: Py<PyFloat>,
    ridley_proficiency: Py<PyFloat>,
    botwoon_proficiency: Py<PyFloat>,
    escape_timer_multiplier: Py<PyFloat>,
    save_animals: Py<PyBool>,
    tech_json: Py<PyString>,
    strat_json: Py<PyString>,
    progression_rate: Py<PyString>,
    item_placement_style: Py<PyString>,
    item_progression_preset: Option<Py<PyString>>,
    objectives: Py<PyString>,
    item_priority_json: Py<PyString>,
    filler_items_json: Py<PyString>,
    race_mode: Py<PyString>,
    random_seed: Py<PyString>,
    quality_of_life_preset: Option<Py<PyBool>>,
    supers_double: Py<PyBool>,
    mother_brain_short: Py<PyBool>,
    escape_enemies_cleared: Py<PyBool>,
    escape_movement_items: Py<PyBool>,
    mark_map_stations: Py<PyBool>,
    item_markers: Py<PyString>,
    all_items_spawn: Py<PyBool>,
    fast_elevators: Py<PyBool>,
    fast_doors: Py<PyBool>,
}

impl From<&PyRandomizeRequest> for RandomizeRequest {
    fn from(py_req: &PyRandomizeRequest) -> Self {
        RandomizeRequest {
            rom: Bytes::from(py_req.rom.as_bytes()),
            preset: Option::from(py_req.preset),
            shinespark_tiles: py_req.shinespark_tiles.to_string().parse().unwrap(),
            resource_multiplier: py_req
*/
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
                    let items = vec![false; rando.game_data.item_isv.keys.len()];
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
                        result = true;
                    }
                }
            }
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

#[pyclass]
#[derive(Clone)]
pub struct APRandomizer {
    #[pyo3(get)]
    randomizer: Randomizer,
    regions_map: Vec<usize>,
    regions_map_reverse: Vec<usize>, 
}

#[pymethods]
impl APRandomizer{
    #[new]
    pub fn new(map_seed: i32) -> Self {
        let sm_json_data_path = Path::new("worlds/sm_map_rando/data/sm-json-data");
        let room_geometry_path = Path::new("worlds/sm_map_rando/data/room_geometry.json");
        let palettes_path = Path::new("worlds/sm_map_rando/data/palettes.json");
        let game_data: GameData = GameData::load(sm_json_data_path, room_geometry_path, palettes_path).unwrap();

        let binding = get_map_repository("worlds/sm_map_rando/data/mapRepository.json").unwrap();
        let map_repository_array = binding.as_slice();
        let map = get_map(Path::new("https://storage.googleapis.com/super-metroid-map-rando/maps/session-2022-06-03T17%3A19%3A29.727911.pkl-bk30-subarea-balance-2/"),
                            map_repository_array,
                            TryInto::<usize>::try_into(map_seed).unwrap()).unwrap();
        
        let difficulty_tiers = vec![get_difficulty_config(&game_data); 1];
        let randomizer = Randomizer::new(Box::new(map), Box::new(difficulty_tiers), Box::new(game_data));

        let (regions_map, regions_map_reverse) = randomizer.game_data.get_regions_map();

        APRandomizer { 
            randomizer,
            regions_map,
            regions_map_reverse,
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

    fn get_event_location_names(&self) -> Vec<String> {
        let mut flag_loc: Vec<String> = Vec::new();
        for &(room_id, node_id, flag_id) in &self.flag_locations {
            let flag_name = self.flag_isv.keys[flag_id].clone();
            println!("{} {} {}", room_id, node_id, flag_name);
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

#[pymodule]
#[pyo3(name = "map_randomizer")]
fn map_randomizer(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Map>()?;
    m.add_class::<GameData>()?;
    m.add_class::<DifficultyConfig>()?;
    m.add_class::<Item>()?;
    m.add_class::<APRandomizer>()?;
    m.add_class::<APCollectionState>()?;
    m.add_wrapped(wrap_pyfunction!(create_gamedata))?;
    Ok(())
}
