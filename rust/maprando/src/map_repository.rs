use anyhow::{Context, Result};
use log::info;
use reqwest::blocking::Client;
use url::Url;
use std::{path::{Path, PathBuf}};

use crate::randomize::Randomizer;
use maprando_game::{GameData, Map};

#[derive(Clone)]
pub struct MapRepository {
    pub base_path: PathBuf,
    pub filenames: Vec<String>,
}

impl MapRepository {
    pub fn new(name: &str, base_path: &Path, game_data: &GameData) -> Result<Self> {
        let mut filenames: Vec<String> = Vec::new();
        if name == "Vanilla" {
            filenames.push("vanilla_map".to_string());
        }
        else {
            let contents = game_data.read_to_string(Path::new(base_path
                .join(if name == "Standard" {"mapRepositoryTame.json"} else {"mapRepositoryWild.json"}).as_path()))?;
            let map_array: Vec<String> = serde_json::from_str(&contents).unwrap();
            for path in map_array {
                filenames.push(path);
            }
        }
        
        filenames.sort();
        info!(
            "{}: {} maps available ({})",
            name,
            filenames.len(),
            base_path.display()
        );
        Ok(MapRepository {
            base_path: if name == "Vanilla" {
                        base_path.to_owned()
                        } 
                        else {
                            Path::new(if name == "Standard" {
                                            "https://storage.googleapis.com/super-metroid-map-rando/maps/v110c-tame/"
                                        } 
                                        else {
                                            "https://storage.googleapis.com/super-metroid-map-rando/maps/v110c-wild/"
                                        }).to_owned()
                        },
            filenames,
        })
    }

    pub fn get_map(
        &self,
        attempt_num_rando: usize,
        seed: usize,
        game_data: &GameData,
        client: &Client
    ) -> Result<Map, anyhow::Error> {
        let idx = seed % self.filenames.len();
        let path = self.base_path.join(&self.filenames[idx]).with_extension("json");
        let mut map = if self.filenames.len() == 1 {
            let map_string = game_data.read_to_string(&path).with_context(|| {
                format!(
                    "[attempt {attempt_num_rando}] Unable to read map file at {}",
                    path.display()
                )
            })?;
            info!("[attempt {attempt_num_rando}] Map: {}", path.display());
            serde_json::from_str(&map_string).with_context(|| {
                format!(
                    "[attempt {attempt_num_rando}] Unable to parse map file at {}",
                    path.display()
                )
            })?
        } else {
            let url = Url::parse(path.to_str().unwrap()).unwrap();
            let response = client.get(url).send()
                .with_context(|| format!("Unable to fetch map file from {}", path.display()))?;
            response.json()
                .with_context(|| format!("Unable to parse map file at {}", path.display()))?
        };
        // Make Toilet area/subarea align with its intersecting room(s):
        // TODO: Push this upstream into the map generation
        let toilet_intersections = Randomizer::get_toilet_intersections(&map, game_data);
        if toilet_intersections.len() > 0 {
            let area = map.area[toilet_intersections[0]];
            let subarea = map.subarea[toilet_intersections[0]];
            for i in 1..toilet_intersections.len() {
                if map.area[toilet_intersections[i]] != area {
                    panic!("Mismatched areas for Toilet intersection");
                }
                if map.subarea[toilet_intersections[i]] != subarea {
                    panic!("Mismatched subareas for Toilet intersection");
                }
            }
            map.area[game_data.toilet_room_idx] = area;
            map.subarea[game_data.toilet_room_idx] = subarea;
        }
        Ok(map)
    }
}
