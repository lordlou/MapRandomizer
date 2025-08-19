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
pub mod upgrade;
pub mod customize_seed;

use log::{info, error};
use pyo3::prelude::*;
use rand::{RngCore, SeedableRng};
use randomize::{filter_links, get_difficulty_tiers, get_objectives, order_map_areas, randomize_doors, randomize_map_areas, DifficultyConfig, Randomization, Randomizer, SpoilerItemSummary, SpoilerLog, SpoilerSummary};
use settings::{AreaAssignment, RandomizerSettings, StartLocationMode};
use std::{path::Path, time::{Instant}};
use upgrade::try_upgrade_settings;
use customize_seed::{customize_seed_ap, CustomizeRequest};
use reqwest::blocking::Client;

use hashbrown::HashMap;
use crate::{
    customize::mosaic::MosaicTheme,
    map_repository::MapRepository,
    preset::PresetData, randomize::{EssentialItemSpoilerInfo, EssentialSpoilerData},
};
use maprando_game::{GameData, Item, LinksDataGroup};

pub const VERSION: usize = include!("VERSION");

#[derive(Clone)]
pub struct VersionInfo {
    pub version: usize,
    pub dev: bool,
}

#[pyclass]
#[derive(Clone)]
pub struct AppData {
    #[pyo3(get)]
    pub game_data: GameData,
    #[pyo3(get)]
    pub preset_data: PresetData,
    pub map_repositories: HashMap<String, MapRepository>,
    //pub seed_repository: SeedRepository,
    //pub visualizer_files: Vec<(String, Vec<u8>)>, // (path, contents)
    //pub video_storage_url: String,
    //pub video_storage_path: Option<String>,
    //pub samus_sprite_categories: Vec<SamusSpriteCategory>,
    //pub logic_data: LogicData,
    //pub _debug: bool,
    //pub port: u16,
    //pub version_info: VersionInfo,
    //pub static_visualizer: bool,
    pub etank_colors: Vec<Vec<String>>, // colors in HTML hex format, e.g "#ff0000"
    pub mosaic_themes: Vec<MosaicTheme>,
}

#[pyfunction]
fn build_app_data(apworld_path: Option<String>) -> AppData {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("error"))
        .format_timestamp_millis()
        .init();

    let start_time = Instant::now();
    let sm_json_data_path = Path::new("worlds/sm_map_rando/data/sm-json-data");
    let room_geometry_path = Path::new("worlds/sm_map_rando/data/room_geometry.json");
    let escape_timings_path = Path::new("worlds/sm_map_rando/data/escape_timings.json");
    let start_locations_path = Path::new("worlds/sm_map_rando/data/start_locations.json");
    let hub_locations_path = Path::new("worlds/sm_map_rando/data/hub_locations.json");
    let etank_colors_path = Path::new("worlds/sm_map_rando/data/etank_colors.json");
    let reduced_flashing_path = Path::new("worlds/sm_map_rando/data/reduced_flashing.json");
    //let strat_videos_path = Path::new("worlds/sm_map_rando/data/strat_videos.json");
    let vanilla_map_path = Path::new("worlds/sm_map_rando/data");
    let standard_maps_path = Path::new("worlds/sm_map_rando/data");
    let wild_maps_path = Path::new("worlds/sm_map_rando/data");
    //let samus_sprites_path = Path::new("../MapRandoSprites/samus_sprites/manifest.json");
    let title_screen_path = Path::new("worlds/sm_map_rando/data/TitleScreen/Images");
    let tech_path = Path::new("worlds/sm_map_rando/data/tech_data.json");
    let notable_path = Path::new("worlds/sm_map_rando/data/notable_data.json");
    let presets_path = Path::new("worlds/sm_map_rando/data/presets");
    let map_tiles_path = Path::new("worlds/sm_map_rando/data/map_tiles.json");
    let mosaic_themes = vec![
        ("OuterCrateria", "Outer Crateria"),
        ("InnerCrateria", "Inner Crateria"),
        ("BlueBrinstar", "Blue Brinstar"),
        ("GreenBrinstar", "Green Brinstar"),
        ("PinkBrinstar", "Pink Brinstar"),
        ("RedBrinstar", "Red Brinstar"),
        ("UpperNorfair", "Upper Norfair"),
        ("LowerNorfair", "Lower Norfair"),
        ("WreckedShip", "Wrecked Ship"),
        ("WestMaridia", "West Maridia"),
        ("YellowMaridia", "Yellow Maridia"),
        ("MechaTourian", "Mecha Tourian"),
        ("MetroidHabitat", "Metroid Habitat"),
    ]
    .into_iter()
    .map(|(x, y)| MosaicTheme {
        name: x.to_string(),
        display_name: y.to_string(),
    })
    .collect();

    let game_data = GameData::load(
        sm_json_data_path,
        room_geometry_path,
        escape_timings_path,
        start_locations_path,
        hub_locations_path,
        title_screen_path,
        reduced_flashing_path,
        map_tiles_path,
        apworld_path
    )
    .unwrap();

    info!("Loading logic preset data");
    let etank_colors: Vec<Vec<String>> =
        serde_json::from_str(&game_data.read_to_string(&etank_colors_path).unwrap()).unwrap();
    //let version_info = VersionInfo {
    //    version: VERSION,
    //    dev: args.dev,
    //};
    //let video_storage_url = if args.video_storage_path.is_some() {
    //    "/videos".to_string()
    //} else {
    //    args.video_storage_url.clone()
    //};

    let preset_data = PresetData::load(tech_path, notable_path, presets_path, &game_data).unwrap();

    //let logic_data = LogicData::new(&game_data, &preset_data, &version_info, &video_storage_url);
    //let samus_sprite_categories: Vec<SamusSpriteCategory> =
    //    serde_json::from_str(&std::fs::read_to_string(&samus_sprites_path).unwrap()).unwrap();

    let map_repositories = vec![
            (
                "Vanilla".to_string(),
                MapRepository::new("Vanilla", vanilla_map_path, &game_data).unwrap(),
            ),
            (
                "Standard".to_string(),
                MapRepository::new("Standard", standard_maps_path, &game_data).unwrap(),
            ),
            (
                "Wild".to_string(),
                MapRepository::new("Wild", wild_maps_path, &game_data).unwrap(),
            )].into_iter().collect();

    let app_data = AppData {
        game_data,
        preset_data,
        map_repositories,
        //seed_repository: SeedRepository::new(&args.seed_repository_url).unwrap(),
        //visualizer_files: load_visualizer_files(),
        //video_storage_url,
        //video_storage_path: args.video_storage_path.clone(),
        //logic_data,
        //samus_sprite_categories,
        //_debug: args.debug,
        //port: args.port,
        //version_info: VersionInfo {
        //    version: VERSION,
        //    dev: args.dev,
        //},
        //static_visualizer: args.static_visualizer,
        etank_colors,
        mosaic_themes,
    };
    info!("Start-up time: {:.3}s", start_time.elapsed().as_secs_f32());
    app_data
}

pub fn get_random_seed() -> usize {
    (rand::rngs::StdRng::from_entropy().next_u64() & 0xFFFFFFFF) as usize
}

#[pyclass]
pub struct AttemptOutput {
        map_seed: usize,
        door_randomization_seed: usize,
        item_placement_seed: usize,
        #[pyo3(get)]
        randomization: Randomization,
        #[pyo3(get)]
        spoiler_log: SpoilerLog,
    }

#[pyfunction]
fn validate_settings_ap(
    rando_settings: String,
    app_data: AppData,
) -> Option<RandomizerSettings> {
    match try_upgrade_settings(rando_settings, &app_data, true) {
        Ok(s) => Some(s.1),
        Err(e) => {
            error!("Failed to upgrade settings: {}", e);
            return None
        }
    }
}

#[pyfunction]
fn randomize_ap(
    mut settings: RandomizerSettings,
    seed: usize,
    map_seed_ap: Option<usize>,
    door_seed_ap: Option<usize>,
    app_data: AppData,
) -> Option<AttemptOutput> {
    let mut validated_preset = false;
    for s in &app_data.preset_data.full_presets {
        if s == &settings {
            validated_preset = true;
            break;
        }
    }
    if !validated_preset {
        settings.name = Some("Custom".to_string());
    }

    let skill_settings = &settings.skill_assumption_settings;
    //let item_settings = &settings.item_progression_settings;
    //let qol_settings = &settings.quality_of_life_settings;
    //let other_settings = &settings.other_settings;
    let race_mode = settings.other_settings.race_mode;
    let random_seed = if race_mode {
        get_random_seed()
    } else {
        if settings.other_settings.random_seed.is_none() {
            seed
        } else {
            settings.other_settings.random_seed.unwrap()
        }
    };
    let display_seed = if race_mode {
        get_random_seed()
    } else {
        random_seed
    };

    if skill_settings.ridley_proficiency < 0.0 || skill_settings.ridley_proficiency > 1.0 {
        info!("Invalid Ridley proficiency");
        return None
    }
    let mut rng_seed = [0u8; 32];
    rng_seed[..8].copy_from_slice(&random_seed.to_le_bytes());
    let mut rng = rand::rngs::StdRng::from_seed(rng_seed);

    let implicit_tech = &app_data.preset_data.tech_by_difficulty["Implicit"];
    let implicit_notables = &app_data.preset_data.notables_by_difficulty["Implicit"];
    let difficulty = DifficultyConfig::new(
        &skill_settings,
        &app_data.game_data,
        &implicit_tech,
        &implicit_notables,
    );
    let difficulty_tiers = get_difficulty_tiers(
        &settings,
        &app_data.preset_data.difficulty_tiers,
        &app_data.game_data,
        &app_data.preset_data.tech_by_difficulty["Implicit"],
        &app_data.preset_data.notables_by_difficulty["Implicit"],
    );

    let filtered_base_links =
        filter_links(&app_data.game_data.links, &app_data.game_data, &difficulty);
    let filtered_base_links_data = LinksDataGroup::new(
        filtered_base_links,
        app_data.game_data.vertex_isv.keys.len(),
        0,
    );
    let map_layout = settings.map_layout.clone();
    let max_attempts = 2000;
    let max_attempts_per_map = if settings.start_location_settings.mode == StartLocationMode::Random
    {
        10
    } else {
        1
    };
    let max_map_attempts = max_attempts / max_attempts_per_map;
    info!(
        "Random seed={random_seed}, max_attempts_per_map={max_attempts_per_map}, max_map_attempts={max_map_attempts}, difficulty={:?}",
        difficulty_tiers[0]
    );

    let time_start_attempts = Instant::now();
    let mut attempt_num = 0;
    let mut output_opt: Option<AttemptOutput> = None;
    let client = Client::new();
    'attempts: for _ in 0..max_map_attempts {
        let map_seed = if map_seed_ap.is_some() {map_seed_ap.unwrap()} else {(rng.next_u64() & 0xFFFFFFFF) as usize};
        let door_randomization_seed = if door_seed_ap.is_some() {door_seed_ap.unwrap()} else {(rng.next_u64() & 0xFFFFFFFF) as usize};

        if !app_data.map_repositories.contains_key(&map_layout) {
            // TODO: it doesn't make sense to panic on things like this.
            panic!("Unrecognized map layout option: {}", map_layout);
        }
        let mut map = app_data.map_repositories[&map_layout]
            .get_map(attempt_num, map_seed, &app_data.game_data, &client)
            .unwrap();
        match settings.other_settings.area_assignment {
            AreaAssignment::Ordered => {
                order_map_areas(&mut map, map_seed, &app_data.game_data);
            }
            AreaAssignment::Random => {
                randomize_map_areas(&mut map, map_seed);
            }
            AreaAssignment::Standard => {}
        }
        let objectives = get_objectives(&settings, &mut rng);
        let locked_door_data = randomize_doors(
            &app_data.game_data,
            &map,
            &settings,
            &objectives,
            door_randomization_seed,
        );
        let randomizer = Randomizer::new(
            &map,
            &locked_door_data,
            objectives.clone(),
            &settings,
            &difficulty_tiers,
            &app_data.game_data,
            &filtered_base_links_data,
            &mut rng,
        );

        for _ in 0..max_attempts_per_map {
            let item_placement_seed = (rng.next_u64() & 0xFFFFFFFF) as usize;
            attempt_num += 1;

            info!("Attempt {attempt_num}/{max_attempts}: Map seed={map_seed}, door randomization seed={door_randomization_seed}, item placement seed={item_placement_seed}");
            let randomization_result =
                randomizer.randomize(attempt_num, item_placement_seed, display_seed);
            let (randomization, spoiler_log) = match randomization_result {
                Ok(x) => x,
                Err(e) => {
                    info!(
                        "Attempt {attempt_num}/{max_attempts}: Randomization failed: {}",
                        e
                    );
                    continue;
                }
            };
            info!(
                "Successful attempt {attempt_num}/{attempt_num}/{max_attempts}: display_seed={}, random_seed={random_seed}, map_seed={map_seed}, door_randomization_seed={door_randomization_seed}, item_placement_seed={item_placement_seed}",
                randomization.display_seed,
            );
            output_opt = Some(AttemptOutput {
                map_seed,
                door_randomization_seed,
                item_placement_seed,
                randomization,
                spoiler_log,
            });
            break 'attempts;
        }
    }

    if output_opt.is_none() {
        info!("Failed too many randomization attempts");
        return None
    }
    //let output = output_opt.unwrap();

    info!(
        "Wall-clock time for attempts: {:?} sec",
        time_start_attempts.elapsed().as_secs_f32()
    );
    output_opt
    //let timestamp = match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
    //    Ok(n) => n.as_millis() as usize,
    //    Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    //};
/*
    let seed_data = SeedData {
        version: VERSION,
        timestamp,
        peer_addr: http_req
            .peer_addr()
            .map(|x| format!("{:?}", x))
            .unwrap_or(String::new()),
        http_headers: format_http_headers(&http_req),
        random_seed: random_seed,
        map_seed: output.map_seed,
        door_randomization_seed: output.door_randomization_seed,
        item_placement_seed: output.item_placement_seed,
        settings: settings.clone(),
        race_mode,
        preset: skill_settings.preset.clone(),
        item_progression_preset: item_settings.preset.clone(),
        difficulty: difficulty_tiers[0].clone(),
        quality_of_life_preset: qol_settings.preset.clone(),
        supers_double: qol_settings.supers_double,
        mother_brain_fight: to_variant_name(&qol_settings.mother_brain_fight)
            .unwrap()
            .to_string(),
        escape_enemies_cleared: qol_settings.escape_enemies_cleared,
        escape_refill: qol_settings.escape_refill,
        escape_movement_items: qol_settings.escape_movement_items,
        mark_map_stations: qol_settings.mark_map_stations,
        transition_letters: other_settings.transition_letters,
        item_markers: to_variant_name(&qol_settings.item_markers)
            .unwrap()
            .to_string(),
        item_dot_change: to_variant_name(&other_settings.item_dot_change)
            .unwrap()
            .to_string(),
        all_items_spawn: qol_settings.all_items_spawn,
        acid_chozo: qol_settings.acid_chozo,
        remove_climb_lava: qol_settings.remove_climb_lava,
        buffed_drops: qol_settings.buffed_drops,
        fast_elevators: qol_settings.fast_elevators,
        fast_doors: qol_settings.fast_doors,
        fast_pause_menu: qol_settings.fast_pause_menu,
        respin: qol_settings.respin,
        infinite_space_jump: qol_settings.infinite_space_jump,
        momentum_conservation: qol_settings.momentum_conservation,
        fanfares: to_variant_name(&qol_settings.fanfares).unwrap().to_string(),
        objectives: output
            .randomization
            .objectives
            .iter()
            .map(|x| to_variant_name(x).unwrap().to_string())
            .collect(),
        doors: to_variant_name(&settings.doors_mode).unwrap().to_string(),
        start_location_mode: if settings.start_location_settings.mode == StartLocationMode::Custom {
            output.randomization.start_location.name.clone()
        } else {
            to_variant_name(&settings.start_location_settings.mode)
                .unwrap()
                .to_string()
        },
        map_layout: settings.map_layout.clone(),
        save_animals: to_variant_name(&settings.save_animals).unwrap().to_string(),
        early_save: qol_settings.early_save,
        area_assignment: to_variant_name(&other_settings.area_assignment)
            .unwrap()
            .to_string(),
        wall_jump: to_variant_name(&other_settings.wall_jump)
            .unwrap()
            .to_string(),
        maps_revealed: to_variant_name(&other_settings.maps_revealed)
            .unwrap()
            .to_string(),
        vanilla_map: settings.map_layout == "Vanilla",
        ultra_low_qol: other_settings.ultra_low_qol,
    };

    let seed_name = &output.randomization.seed_name;
    save_seed(
        seed_name,
        &seed_data,
        &req.settings.0,
        &req.spoiler_token.0,
        &settings,
        &output.randomization,
        &output.spoiler_log,
        &app_data,
    )
    .await
    .unwrap();

    HttpResponse::Ok().json(RandomizeResponse {
        seed_url: format!("/seed/{}/", seed_name),
    })
*/
}

#[pymodule]
#[pyo3(name = "pysmmaprando")]
fn pysmmaprando(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Item>()?;
    m.add_class::<GameData>()?;
    m.add_class::<AttemptOutput>()?;
    m.add_class::<AppData>()?;
    m.add_class::<CustomizeRequest>()?;
    m.add_class::<RandomizerSettings>()?;

    m.add_function(wrap_pyfunction!(build_app_data, m)?)?;
    m.add_function(wrap_pyfunction!(validate_settings_ap, m)?)?;
    m.add_function(wrap_pyfunction!(randomize_ap, m)?)?;
    m.add_function(wrap_pyfunction!(customize_seed_ap, m)?)?;
    Ok(())
}
