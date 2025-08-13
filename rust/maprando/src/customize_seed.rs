use crate::AppData;
use log::info;
use crate::{
    customize::{
        customize_rom, parse_controller_button, ControllerButton, ControllerConfig,
        CustomizeSettings, DoorTheme, FlashingSetting, MusicSettings, PaletteTheme, ShakingSetting,
        TileTheme,
    },
    patch::{make_rom, Rom},
    randomize::Randomization,
    settings::RandomizerSettings,
};
use maprando_game::{Item, Map};
use pyo3::prelude::*;

#[derive(Clone)]
#[pyclass]
pub struct CustomizeRequest {
    rom: Vec<u8>,
    samus_sprite: String,
    etank_color: String,
    reserve_hud_style: bool,
    room_palettes: String,
    tile_theme: String,
    door_theme: String,
    music: String,
    disable_beeping: bool,
    shaking: String,
    flashing: String,
    vanilla_screw_attack_animation: bool,
    control_shot: String,
    control_jump: String,
    control_dash: String,
    control_item_select: String,
    control_item_cancel: String,
    control_angle_up: String,
    control_angle_down: String,
    spin_lock_left: Option<String>,
    spin_lock_right: Option<String>,
    spin_lock_up: Option<String>,
    spin_lock_down: Option<String>,
    spin_lock_x: Option<String>,
    spin_lock_y: Option<String>,
    spin_lock_a: Option<String>,
    spin_lock_b: Option<String>,
    spin_lock_l: Option<String>,
    spin_lock_r: Option<String>,
    spin_lock_select: Option<String>,
    spin_lock_start: Option<String>,
    quick_reload_left: Option<String>,
    quick_reload_right: Option<String>,
    quick_reload_up: Option<String>,
    quick_reload_down: Option<String>,
    quick_reload_x: Option<String>,
    quick_reload_y: Option<String>,
    quick_reload_a: Option<String>,
    quick_reload_b: Option<String>,
    quick_reload_l: Option<String>,
    quick_reload_r: Option<String>,
    quick_reload_select: Option<String>,
    quick_reload_start: Option<String>,
    moonwalk: bool,
}

#[pymethods]
impl CustomizeRequest{
    #[new]
    pub fn new(rom: Vec<u8>,
        samus_sprite: String,
        etank_color: String,
        reserve_hud_style: bool,
        room_palettes: String,
        tile_theme: String,
        door_theme: String,
        music: String,
        disable_beeping: bool,
        shaking: String,
        flashing: String,
        vanilla_screw_attack_animation: bool,
        control_shot: String,
        control_jump: String,
        control_dash: String,
        control_item_select: String,
        control_item_cancel: String,
        control_angle_up: String,
        control_angle_down: String,
        spin_lock_left: Option<String>,
        spin_lock_right: Option<String>,
        spin_lock_up: Option<String>,
        spin_lock_down: Option<String>,
        spin_lock_x: Option<String>,
        spin_lock_y: Option<String>,
        spin_lock_a: Option<String>,
        spin_lock_b: Option<String>,
        spin_lock_l: Option<String>,
        spin_lock_r: Option<String>,
        spin_lock_select: Option<String>,
        spin_lock_start: Option<String>,
        quick_reload_left: Option<String>,
        quick_reload_right: Option<String>,
        quick_reload_up: Option<String>,
        quick_reload_down: Option<String>,
        quick_reload_x: Option<String>,
        quick_reload_y: Option<String>,
        quick_reload_a: Option<String>,
        quick_reload_b: Option<String>,
        quick_reload_l: Option<String>,
        quick_reload_r: Option<String>,
        quick_reload_select: Option<String>,
        quick_reload_start: Option<String>,
        moonwalk: bool) -> Self {
        CustomizeRequest {
            rom,
            samus_sprite,
            etank_color,
            reserve_hud_style,
            room_palettes,
            tile_theme,
            door_theme,
            music,
            disable_beeping,
            shaking,
            flashing,
            vanilla_screw_attack_animation,
            control_shot,
            control_jump,
            control_dash,
            control_item_select,
            control_item_cancel,
            control_angle_up,
            control_angle_down,
            spin_lock_left,
            spin_lock_right,
            spin_lock_up,
            spin_lock_down,
            spin_lock_x,
            spin_lock_y,
            spin_lock_a,
            spin_lock_b,
            spin_lock_l,
            spin_lock_r,
            spin_lock_select,
            spin_lock_start,
            quick_reload_left,
            quick_reload_right,
            quick_reload_up,
            quick_reload_down,
            quick_reload_x,
            quick_reload_y,
            quick_reload_a,
            quick_reload_b,
            quick_reload_l,
            quick_reload_r,
            quick_reload_select,
            quick_reload_start,
            moonwalk
        }
    }
}

#[pyfunction]
pub fn customize_seed_ap(
    req: CustomizeRequest,
    app_data: AppData,
    settings: Option<RandomizerSettings>,
    mut randomization: Option<Randomization>,
    map: Map,
    ultra_low_qol: bool,
    new_item_placement: Vec<Item>
) -> Vec<u8> {
    info!("customize_seed_ap");
    //let seed_name = &info.0;
    let orig_rom = Rom::new(req.rom.clone());
    let mut rom = orig_rom.clone();
/*
    let seed_data_str: String = String::from_utf8(
        app_data
            .seed_repository
            .get_file(seed_name, "seed_data.json")
            .await
            .unwrap(),
    )
    .unwrap();
    let seed_data = json::parse(&seed_data_str).unwrap();

    let map_data_bytes = app_data
        .seed_repository
        .get_file(seed_name, "map.json")
        .await
        .unwrap_or(vec![]);
    let map: Option<Map> = if map_data_bytes.len() == 0 {
        None
    } else {
        Some(serde_json::from_slice(&map_data_bytes).unwrap())
    };

    let settings_bytes = app_data
        .seed_repository
        .get_file(seed_name, "public/settings.json")
        .await
        .unwrap_or(vec![]);
    let settings: Option<RandomizerSettings> = if settings_bytes.len() == 0 {
        None
    } else {
        match try_upgrade_settings(String::from_utf8(settings_bytes).unwrap(), &app_data, false) {
            Ok(s) => Some(s.1),
            Err(e) => {
                return HttpResponse::InternalServerError().body(e.to_string());
            }
        }
    };

    let randomization_bytes = app_data
        .seed_repository
        .get_file(seed_name, "randomization.json")
        .await
        .unwrap_or(vec![]);
    let randomization: Option<Randomization> = if randomization_bytes.len() == 0 {
        None
    } else {
        Some(serde_json::from_slice(&randomization_bytes).unwrap())
    };

    let ultra_low_qol = seed_data["ultra_low_qol"].as_bool().unwrap_or(false);

    let rom_digest = crypto_hash::hex_digest(crypto_hash::Algorithm::SHA256, &rom.data);
    info!("Rom digest: {rom_digest}");
    if rom_digest != "12b77c4bc9c1832cee8881244659065ee1d84c70c3d29e6eaf92e6798cc2ca72" {
        return HttpResponse::BadRequest().body(InvalidRomTemplate {}.render().unwrap());
    }
*/
    let customize_settings = CustomizeSettings {
        samus_sprite: if ultra_low_qol
            && req.samus_sprite == "samus_vanilla"
            && req.vanilla_screw_attack_animation
        {
            None
        } else {
            Some(req.samus_sprite.clone())
        },
        etank_color: Some((
            u8::from_str_radix(&req.etank_color[0..2], 16).unwrap() / 8,
            u8::from_str_radix(&req.etank_color[2..4], 16).unwrap() / 8,
            u8::from_str_radix(&req.etank_color[4..6], 16).unwrap() / 8,
        )),
        reserve_hud_style: req.reserve_hud_style,
        vanilla_screw_attack_animation: req.vanilla_screw_attack_animation,
        palette_theme: if req.room_palettes == "area_themed" {
            PaletteTheme::AreaThemed
        } else {
            PaletteTheme::Vanilla
        },
        tile_theme: if req.tile_theme == "None" {
            TileTheme::Vanilla
        } else if req.tile_theme == "Scrambled" {
            TileTheme::Scrambled
        } else if req.tile_theme == "AreaThemed" {
            TileTheme::AreaThemed
        } else {
            TileTheme::Constant(req.tile_theme.to_string())
        },
        door_theme: match req.door_theme.as_str() {
            "vanilla" => DoorTheme::Vanilla,
            "alternate" => DoorTheme::Alternate,
            _ => panic!(
                "Unexpected door_theme option: {}",
                req.door_theme.as_str()
            ),
        },
        music: match req.music.as_str() {
            "area" => MusicSettings::AreaThemed,
            "disabled" => MusicSettings::Disabled,
            _ => panic!("Unexpected music option: {}", req.music.as_str()),
        },
        disable_beeping: req.disable_beeping,
        shaking: match req.shaking.as_str() {
            "Vanilla" => ShakingSetting::Vanilla,
            "Reduced" => ShakingSetting::Reduced,
            "Disabled" => ShakingSetting::Disabled,
            _ => panic!("Unexpected shaking option: {}", req.shaking.as_str()),
        },
        flashing: match req.flashing.as_str() {
            "Vanilla" => FlashingSetting::Vanilla,
            "Reduced" => FlashingSetting::Reduced,
            _ => panic!("Unexpected flashing option: {}", req.flashing.as_str()),
        },
        controller_config: ControllerConfig {
            shot: parse_controller_button(&req.control_shot).unwrap(),
            jump: parse_controller_button(&req.control_jump).unwrap(),
            dash: parse_controller_button(&req.control_dash).unwrap(),
            item_select: parse_controller_button(&req.control_item_select).unwrap(),
            item_cancel: parse_controller_button(&req.control_item_cancel).unwrap(),
            angle_up: parse_controller_button(&req.control_angle_up).unwrap(),
            angle_down: parse_controller_button(&req.control_angle_down).unwrap(),
            spin_lock_buttons: get_spin_lock_buttons(&req),
            quick_reload_buttons: get_quick_reload_buttons(&req),
            moonwalk: req.moonwalk,
        },
    };

    if settings.is_some() && randomization.is_some() {
        info!("Patching ROM");
        randomization.as_mut().unwrap().item_placement = new_item_placement;
        match make_rom(
            &rom,
            settings.as_ref().unwrap(),
            randomization.as_ref().unwrap(),
            &app_data.game_data,
        ) {
            Ok(r) => {
                rom = r;
            }
            Err(err) => {
                info!("Error patching ROM: {:?}", err);
                return Vec::new()
            }
        }
    } else {
        info!("Seed incompatible with current customizer");
        return Vec::new()
    }

    info!("CustomizeSettings: {:?}", customize_settings);
    match customize_rom(
        &mut rom,
        &orig_rom,
        &Some(map),
        &customize_settings,
        &app_data.game_data,
        //&app_data.samus_sprite_categories,
        &app_data.mosaic_themes,
    ) {
        Ok(()) => {}
        Err(err) => {
            info!("Error customizing ROM: {:?}", err);
            return Vec::new()
        }
    }
    rom.data
}

fn get_spin_lock_buttons(req: &CustomizeRequest) -> Vec<ControllerButton> {
    let mut spin_lock_buttons = vec![];
    let setting_button_mapping = vec![
        (&req.spin_lock_left, ControllerButton::Left),
        (&req.spin_lock_right, ControllerButton::Right),
        (&req.spin_lock_up, ControllerButton::Up),
        (&req.spin_lock_down, ControllerButton::Down),
        (&req.spin_lock_a, ControllerButton::A),
        (&req.spin_lock_b, ControllerButton::B),
        (&req.spin_lock_x, ControllerButton::X),
        (&req.spin_lock_y, ControllerButton::Y),
        (&req.spin_lock_l, ControllerButton::L),
        (&req.spin_lock_r, ControllerButton::R),
        (&req.spin_lock_select, ControllerButton::Select),
        (&req.spin_lock_start, ControllerButton::Start),
    ];

    for (setting, button) in setting_button_mapping {
        if let Some(x) = setting {
            if x == "on" {
                spin_lock_buttons.push(button);
            }
        }
    }
    spin_lock_buttons
}

fn get_quick_reload_buttons(req: &CustomizeRequest) -> Vec<ControllerButton> {
    let mut quick_reload_buttons = vec![];
    let setting_button_mapping = vec![
        (&req.quick_reload_left, ControllerButton::Left),
        (&req.quick_reload_right, ControllerButton::Right),
        (&req.quick_reload_up, ControllerButton::Up),
        (&req.quick_reload_down, ControllerButton::Down),
        (&req.quick_reload_a, ControllerButton::A),
        (&req.quick_reload_b, ControllerButton::B),
        (&req.quick_reload_x, ControllerButton::X),
        (&req.quick_reload_y, ControllerButton::Y),
        (&req.quick_reload_l, ControllerButton::L),
        (&req.quick_reload_r, ControllerButton::R),
        (&req.quick_reload_select, ControllerButton::Select),
        (&req.quick_reload_start, ControllerButton::Start),
    ];

    for (setting, button) in setting_button_mapping {
        if let Some(x) = setting {
            if x == "on" {
                quick_reload_buttons.push(button);
            }
        }
    }
    quick_reload_buttons
}
