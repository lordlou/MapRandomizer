pub mod game_data;
pub mod traverse;
pub mod randomize;
pub mod patch;
pub mod spoiler_map;
pub mod seed_repository;
pub mod web;
pub mod customize;

use pyo3::prelude::*;
use crate::{
    game_data::{GameData, Map, IndexedVec},
    randomize::{Randomizer, get_difficulty_config, DifficultyConfig}
};
use std::path::{Path, PathBuf};
use std::fs;
use reqwest::blocking::{get};
use anyhow::{Context, Result};
use serde_derive::Deserialize;
use url::Url;

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
#[pyclass]
pub struct APRandomizer {
    #[pyo3(get)]
    randomizer: Randomizer,
}

#[pymethods]
impl APRandomizer{
    #[new]
    pub fn new(map_seed: i32) -> Self {
        let sm_json_data_path = Path::new("worlds/sm_map_rando/data/sm-json-data");
        let room_geometry_path = Path::new("worlds/sm_map_rando/data/room_geometry.json");
        let palettes_path = Path::new("worlds/sm_map_rando/data/palettes.json");
        let game_data = GameData::load(sm_json_data_path, room_geometry_path, palettes_path).unwrap();

        let binding = get_map_repository("worlds/sm_map_rando/data/mapRepository.json").unwrap();
        let map_repository_array = binding.as_slice();
        let map = get_map(Path::new("https://storage.googleapis.com/super-metroid-map-rando/maps/session-2022-06-03T17%3A19%3A29.727911.pkl-bk30-subarea-balance-2/"),
                            map_repository_array,
                            TryInto::<usize>::try_into(map_seed).unwrap()).unwrap();
        
        let difficulty_tiers = vec![get_difficulty_config(&game_data); 1];
        let randomizer = Randomizer::new(Box::new(map), Box::new(difficulty_tiers), Box::new(game_data));
        APRandomizer { randomizer }
    }

    fn create_randomizer(&mut self/*map: Map, difficulty_tiers: [DifficultyConfig]*/) {
        let sm_json_data_path = Path::new("worlds/sm_map_rando/data/sm-json-data");
        let room_geometry_path = Path::new("worlds/sm_map_rando/data/room_geometry.json");
        let palettes_path = Path::new("worlds/sm_map_rando/data/palettes.json");
        let game_data = GameData::load(sm_json_data_path, room_geometry_path, palettes_path).unwrap();

        let binding = get_map_repository("worlds/sm_map_rando/data/mapRepository.json").unwrap();
        let map_repository_array = binding.as_slice();
        let map = get_map(Path::new("https://storage.googleapis.com/super-metroid-map-rando/maps/session-2022-06-03T17%3A19%3A29.727911.pkl-bk30-subarea-balance-2/"),
                            map_repository_array,
                            12345).unwrap();
        
        let difficulty_tiers = vec![get_difficulty_config(&game_data); 1];
        self.randomizer = Randomizer::new(Box::new(map), Box::new(difficulty_tiers), Box::new(game_data));
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

#[pymodule]
#[pyo3(name = "map_randomizer")]
fn map_randomizer(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<Map>()?;
    m.add_class::<GameData>()?;
    m.add_class::<DifficultyConfig>()?;
    m.add_class::<APRandomizer>()?;
    Ok(())
}
