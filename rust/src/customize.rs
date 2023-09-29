pub mod retiling;
pub mod room_palettes;
pub mod vanilla_music;

use anyhow::{bail, Result};
use std::path::Path;

use crate::customize::vanilla_music::override_music;
use crate::{
    game_data::GameData,
    patch::{apply_ips_patch, snes2pc, write_credits_big_char, Rom},
    web::SamusSpriteCategory,
};
use retiling::apply_retiling;
use room_palettes::apply_area_themed_palettes;

struct AllocatorBlock {
    start_addr: usize,
    end_addr: usize,
    current_addr: usize,
}

struct Allocator {
    blocks: Vec<AllocatorBlock>,
}

impl Allocator {
    pub fn new(blocks: Vec<(usize, usize)>) -> Self {
        Allocator {
            blocks: blocks
                .into_iter()
                .map(|(start, end)| AllocatorBlock {
                    start_addr: start,
                    end_addr: end,
                    current_addr: start,
                })
                .collect(),
        }
    }

    pub fn allocate(&mut self, size: usize) -> Result<usize> {
        for block in &mut self.blocks {
            if block.end_addr - block.current_addr >= size {
                let addr = block.current_addr;
                block.current_addr += size;
                // println!("success: allocated {} bytes: ending at {:x}", size, pc2snes(block.current_addr));
                return Ok(addr);
            }
        }
        bail!("Failed to allocate {} bytes", size);
    }
}

#[derive(Debug)]
pub enum MusicSettings {
    Vanilla,
    AreaThemed,
    Disabled,
}

#[derive(Debug)]
pub enum AreaTheming {
    Vanilla,
    Palettes,
    Tiles(String),
}

#[derive(Debug)]
pub struct CustomizeSettings {
    pub samus_sprite: Option<String>,
    pub vanilla_screw_attack_animation: bool,
    pub area_theming: AreaTheming,
    pub music: MusicSettings,
    pub disable_beeping: bool,
    pub etank_color: Option<(u8, u8, u8)>,
}

fn remove_mother_brain_flashing(rom: &mut Rom) -> Result<()> {
    // Disable start of flashing after Mother Brain 1:
    rom.write_u16(snes2pc(0xA9CFFE), 0)?;

    // Disable end of flashing (to prevent palette from getting overwritten)
    rom.write_u8(snes2pc(0xA9D00C), 0x60)?; // RTS

    Ok(())
}

fn apply_custom_samus_sprite(
    rom: &mut Rom,
    settings: &CustomizeSettings,
    samus_sprite_categories: &[SamusSpriteCategory],
) -> Result<()> {
    if settings.samus_sprite.is_some() || !settings.vanilla_screw_attack_animation {
        let sprite_name = settings.samus_sprite.clone().unwrap_or("samus".to_string());
        let patch_path_str = format!("../patches/samus_sprites/{}.ips", sprite_name);
        apply_ips_patch(rom, Path::new(&patch_path_str))?;

        if settings.vanilla_screw_attack_animation {
            // Disable spin attack animation, to make it behave like vanilla: Screw attack animation will look like
            // you have Space Jump even if you don't:
            rom.write_u16(snes2pc(0x9B93FE), 0)?;
        }
    }

    // Patch credits to give credit to the sprite author:
    if let Some(sprite_name) = settings.samus_sprite.as_ref() {
        for category in samus_sprite_categories {
            for info in &category.sprites {
                if &info.name == sprite_name {
                    // Write the sprite name
                    let mut chars = vec![];
                    let credits_name = info
                        .credits_name
                        .clone()
                        .unwrap_or(info.display_name.clone());
                    for c in credits_name.chars() {
                        let c = c.to_ascii_uppercase();
                        if (c >= 'A' && c <= 'Z') || c == ' ' {
                            chars.push(c);
                        }
                    }
                    chars.extend(" SPRITE".chars());
                    let mut addr =
                        snes2pc(0xceb240 + (234 - 128) * 0x40) + 0x20 - (chars.len() + 1) / 2 * 2;
                    for c in chars {
                        let color_palette = 0x0400;
                        if c >= 'A' && c <= 'Z' {
                            rom.write_u16(addr, (c as isize - 'A' as isize) | color_palette)?;
                        }
                        addr += 2;
                    }

                    // Write the sprite author
                    let mut chars = vec![];
                    let author = info.authors.join(", ");
                    for c in author.chars() {
                        let c = c.to_ascii_uppercase();
                        if (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == ' ' {
                            chars.push(c);
                        }
                    }
                    let mut addr =
                        snes2pc(0xceb240 + (235 - 128) * 0x40) + 0x20 - (chars.len() + 1) / 2 * 2;
                    for c in chars {
                        write_credits_big_char(rom, c, addr)?;
                        addr += 2;
                    }
                }
            }
        }
    }

    Ok(())
}

pub fn customize_rom(
    rom: &mut Rom,
    seed_patch: &[u8],
    settings: &CustomizeSettings,
    game_data: &GameData,
    samus_sprite_categories: &[SamusSpriteCategory],
) -> Result<()> {
    rom.resize(0x400000);
    let patch = ips::Patch::parse(seed_patch).unwrap();
    // .with_context(|| format!("Unable to parse patch {}", patch_path.display()))?;
    for hunk in patch.hunks() {
        rom.write_n(hunk.offset(), hunk.payload())?;
    }

    remove_mother_brain_flashing(rom)?;
    match &settings.area_theming {
        AreaTheming::Vanilla => {}
        AreaTheming::Palettes => {
            apply_area_themed_palettes(rom, game_data)?;
        }
        AreaTheming::Tiles(theme) => {
            apply_retiling(rom, game_data, &theme)?;
            // // Failed attempt to put Dachora further back, e.g. so it doesn't go in front of Crateria tube:
            // rom.write_u8(snes2pc(0xA0E5FF + 0x39), 0x06)?;
        }
    }
    apply_custom_samus_sprite(rom, settings, samus_sprite_categories)?;
    match settings.music {
        MusicSettings::Vanilla => {
            override_music(rom)?;
        }
        MusicSettings::AreaThemed => {}
        MusicSettings::Disabled => {
            override_music(rom)?;
            rom.write_u8(snes2pc(0xcf8413), 0x6F)?;
        }
    }
    if settings.disable_beeping {
        rom.write_n(snes2pc(0x90EA92), &[0xEA; 4])?;
        rom.write_n(snes2pc(0x90EAA0), &[0xEA; 4])?;
        rom.write_n(snes2pc(0x90F33C), &[0xEA; 4])?;
        rom.write_n(snes2pc(0x91E6DA), &[0xEA; 4])?;
    }
    if let Some((r, g, b)) = settings.etank_color {
        let color = (r as isize) | ((g as isize) << 5) | ((b as isize) << 10);
        rom.write_u16(snes2pc(0x82FFFE), color)?; // Gameplay ETank color
                                                  // rom.write_u16(snes2pc(0xB6F01A), color)?;
        rom.write_u16(snes2pc(0x8EE416), color)?; // Main menu
        rom.write_u16(snes2pc(0xA7CA7B), color)?; // During Phantoon power-on
    }
    Ok(())
}
