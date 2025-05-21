pub mod customize;
pub mod helpers;
pub mod map_repository;
pub mod patch;
pub mod preset;
pub mod randomize;
pub mod seed_repository;
pub mod settings;
pub mod spoiler_map;
pub mod traverse;

use pyo3::prelude::*;
use maprando_game::GameData;
use std::path::Path;

#[pyfunction]
fn create_gamedata(apworld_path: Option<String>) -> GameData {
    let sm_json_data_path = Path::new("worlds/sm_map_rando/data/sm-json-data");
    let room_geometry_path = Path::new("worlds/sm_map_rando/data/room_geometry.json");
    let escape_timings_path = Path::new("worlds/sm_map_rando/data/escape_timings.json");
    let start_locations_path = Path::new("worlds/sm_map_rando/data/start_locations.json");
    let hub_locations_path = Path::new("worlds/sm_map_rando/data/hub_locations.json");
    let title_screen_path = Path::new("worlds/sm_map_rando/data/TitleScreen/Images");
    let reduced_flashing_path = Path::new("worlds/sm_map_rando/data/reduced_flashing.json");
    let strat_videos_path = Path::new("worlds/sm_map_rando/data/strat_videos.json");
    let map_tile_path = Path::new("worlds/sm_map_rando/data/map_tiles.json");

    GameData::load(
        sm_json_data_path, 
        room_geometry_path, 
        escape_timings_path,
        start_locations_path,
        hub_locations_path,
        title_screen_path,
        reduced_flashing_path,
        strat_videos_path,
        map_tile_path,
        apworld_path).unwrap()
}

#[pymodule]
#[pyo3(name = "pysmmaprando")]
fn pysmmaprando(m: &Bound<'_, PyModule>) -> PyResult<()> {
    //m.add_class::<Map>()?;
    //m.add_class::<GameData>()?;
    //m.add_class::<DifficultyConfig>()?;
    //m.add_class::<APRandomizer>()?;
    //m.add_class::<Options>()?;

    m.add_function(wrap_pyfunction!(create_gamedata, m)?)?;
    //m.add_wrapped(wrap_pyfunction!(patch_rom))?;
    Ok(())
}
