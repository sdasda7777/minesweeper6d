// hide console window on Windows in release (also disables console output)
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
#![feature(step_trait)]

extern crate eframe;
extern crate hhmmss;
extern crate itertools;
extern crate toml;
extern crate rand_chacha;

use itertools::Itertools;
use hhmmss::Hhmmss;

pub mod bwi;
use bwi::BWI;

pub mod minesweeper_model;
use minesweeper_model::{CellState, DIMENSIONS_COUNT, GameBoard, GameState, InitialGameSettings};

use eframe::{egui, emath::Align2};
use eframe::egui::{Button, containers::panel::TopBottomPanel, Key, KeyboardShortcut, 
                   menu, Modifiers, PointerButton, Response, RichText, Sense};
use eframe::epaint::{Color32, FontId, Pos2, Rect, Rounding, Shadow, Shape, Stroke};
use std::{cmp::min, fs};
use web_time::SystemTime;
use toml::Table;

#[derive(PartialEq)]
enum CursorMode {
    ProbeAndMark,
    Highlighter,
}

pub struct Shortcuts {
    probe_mark_shortcut: KeyboardShortcut,
    highlighter_shortcut: KeyboardShortcut,
    highlight_group_shortcuts: [KeyboardShortcut; 8],
    
    reset_view_shortcut: KeyboardShortcut,
    zoom_to_fit_shortcut: KeyboardShortcut,
}

impl Shortcuts {
    pub fn new() -> Self {
        let mod_none = Modifiers{
            alt: false,
            ctrl: false,
            shift: false,
            mac_cmd: false,
            command: false,
        };
        Self {
            probe_mark_shortcut: KeyboardShortcut::new(mod_none, Key::Q),
            highlighter_shortcut: KeyboardShortcut::new(mod_none, Key::W),
            highlight_group_shortcuts: [KeyboardShortcut::new(mod_none, Key::Num1),
                                        KeyboardShortcut::new(mod_none, Key::Num2),
                                        KeyboardShortcut::new(mod_none, Key::Num3),
                                        KeyboardShortcut::new(mod_none, Key::Num4),
                                        KeyboardShortcut::new(mod_none, Key::Num5),
                                        KeyboardShortcut::new(mod_none, Key::Num6),
                                        KeyboardShortcut::new(mod_none, Key::Num7),
                                        KeyboardShortcut::new(mod_none, Key::Num8)],
            
            reset_view_shortcut: KeyboardShortcut::new(mod_none, Key::D),
            zoom_to_fit_shortcut: KeyboardShortcut::new(mod_none, Key::F),
        }
    }
}

// When compiling natively:
#[cfg(not(target_arch = "wasm32"))]
fn main() -> Result<(), eframe::Error> {
    let options = eframe::NativeOptions {
        ..Default::default()
    };
    let config_content = fs::read_to_string("config.toml").unwrap_or_else(|_| "".into());

    eframe::run_native(
        "Minesweeper6D",
        options,
        Box::new(|cc| {
            cc.egui_ctx.style_mut(|style| {
                style.visuals.menu_rounding = Rounding::ZERO;
                style.visuals.popup_shadow = Shadow::NONE;
                style.visuals.window_rounding = Rounding::ZERO;
                style.visuals.window_shadow = Shadow::NONE;
            });
            Box::new(MinesweeperViewController::new(config_content))
        }),
    )
}

// When compiling to web using trunk:
#[cfg(target_arch = "wasm32")]
fn main() {
    // Redirect `log` message to `console.log` and friends:
    eframe::WebLogger::init(log::LevelFilter::Debug).ok();

    let web_options = eframe::WebOptions::default();
    let config_content = "".into();
    
    wasm_bindgen_futures::spawn_local(async {
        eframe::WebRunner::new()
            .start(
                "the_canvas_id", // hardcode it
                web_options,
                Box::new(|cc| {
                    cc.egui_ctx.style_mut(|style| {
                        style.visuals.menu_rounding = Rounding::ZERO;
                        style.visuals.popup_shadow = Shadow::NONE;
                        style.visuals.window_rounding = Rounding::ZERO;
                        style.visuals.window_shadow = Shadow::NONE;
                    });
                    Box::new(MinesweeperViewController::new(config_content))
                }),
            )
            .await
            .expect("failed to start eframe");
    });
}

struct MinesweeperViewController {
    current_initial_settings: InitialGameSettings,
    next_initial_settings: InitialGameSettings,

    next_selected_preset: Option<u32>,
    presets: Vec<InitialGameSettings>,

    game: Option<GameBoard>,
    start_time: Option<SystemTime>,
    end_time: Option<SystemTime>,
    
    cursor_mode: CursorMode,
    selected_highlighters: u8,
    
    view_origin: Pos2,
    zoom_factor: f32,
    cell_edge: f32,
    tile_spacings: [f32; DIMENSIONS_COUNT],
    
    show_timer_miliseconds: bool,
    show_delta: bool,
    show_neighbors: bool,
    unlimited_zoom: bool,
    probe_marked: bool,
    neighbor_coords: Option<[usize; DIMENSIONS_COUNT]>,
    
    new_game_window_enabled: bool,
    rules_window_enabled: bool,
    controls_window_enabled: bool,
    about_window_enabled: bool,
    
    selection_color: Color32,
    center_color: Color32,
    neighbor_color: Color32,
    highlight_colors: [Color32; 8],
    
    shortcuts: Shortcuts,
}

impl MinesweeperViewController {
    fn new(config_text: String) -> Self {
        // Sanity check
        //println!("{}", std::mem::size_of::<CellState>());

        let settings = InitialGameSettings {
            name: "Custom".into(),
            size: [4, 4, 4, 4, 1, 1],
            wrap: [false, false, false, false, false, false],
            mines: 20,
            seed: None,
        };
        
        let mut ret = Self {
            current_initial_settings: settings.clone(),
            next_initial_settings: settings,
            
            next_selected_preset: None,
            presets: vec![],
            
            game: None,
            start_time: None,
            end_time: None,
            
            cursor_mode: CursorMode::ProbeAndMark,
            selected_highlighters: 1,
            
            view_origin: Pos2::new(0.0, 20.0),
            zoom_factor: 1.0,
            cell_edge: 30.0,
            tile_spacings: [0.0, 0.0, 10.0, 10.0, 20.0, 20.0],
            
            show_timer_miliseconds: false,
            show_delta: true,
            show_neighbors: true,
            unlimited_zoom: false,
            probe_marked: false,
            neighbor_coords: None,
            
            new_game_window_enabled: false,
            rules_window_enabled: false,
            controls_window_enabled: false,
            about_window_enabled: false,
            
            selection_color: Color32::RED,
            center_color: Color32::LIGHT_RED,
            neighbor_color: Color32::LIGHT_BLUE,
            highlight_colors: [Color32::YELLOW, Color32::BROWN, Color32::LIGHT_GREEN, Color32::WHITE,
                               Color32::KHAKI, Color32::DARK_BLUE, Color32::DARK_GREEN, Color32::GOLD],
            
            shortcuts: Shortcuts::new(),
        };

        let config_table: Table = config_text.parse::<Table>().expect("Invalid configuration file");
        // Load in presets
        if let Some(val) = config_table.get("preset"){
            for e in val.as_array().unwrap() {
                let mut igs: InitialGameSettings = Default::default();
                if let Some(n) = e.get("name") {
                    igs.name = n.as_str().unwrap().into();
                }
                if let Some(size_value) = e.get("size") {
                    if let Some(a) = size_value.as_array() {
                        if a.len() != DIMENSIONS_COUNT {
                            println!("Warning: `size` should be array of {} elements, {} found", DIMENSIONS_COUNT, a.len());
                        }
                        for ii in 0..min(a.len(), DIMENSIONS_COUNT) {
                            if let Some(i) = a[ii].as_integer().map(|e| e.clamp(1, 100) as usize) {
                                igs.size[ii] = i;
                            } else {
                                println!("Warning: value at index {} of `size` is invalid", ii);
                            }
                        }
                    } else {
                        println!("Warning: value of `size` is invalid");
                    }
                }
                if let Some(wrap_value) = e.get("wrap") {
                    if let Some(a) = wrap_value.as_array() {
                        if a.len() != DIMENSIONS_COUNT {
                            println!("Warning: `wrap` should be array of {} elements, {} found", DIMENSIONS_COUNT, a.len());
                        }
                        for ii in 0..min(a.len(), DIMENSIONS_COUNT) {
                            if let Some(b) = a[ii].as_bool() {
                                igs.wrap[ii] = b;
                            } else {
                                println!("Warning: value at index {} of `wrap` is invalid", ii);
                            }
                        }
                    } else {
                        println!("Warning: value of `wrap` is invalid");
                    }
                }
                if let Some(mines_value) = e.get("mines") {
                    if let Some(i) = mines_value.as_integer()
                                        .map(|e| (e as u32)
                                                .clamp(1, (igs.size.iter().fold(1, |p, v| p*v) - 1) as u32)) {
                        igs.mines = i;
                    } else {
                        println!("Warning: value of `mines` is invalid");
                    }
                }
                if let Some(seed_value) = e.get("seed") {
                    if let Some(s) = seed_value.as_str() {
                        igs.seed = Some(s.into());
                    } else {
                        println!("Warning: value of `seed` is invalid");
                    }
                }
                ret.presets.push(igs);
            }
        }
        
        // Load in other settings
        if let Some(val) = config_table.get("default_preset") {
            if let Some(i) = val.as_integer().map(|e| e as usize) {
                if i < ret.presets.len() {
                    ret.current_initial_settings = ret.presets[i].clone();
                    ret.next_initial_settings = ret.presets[i].clone();
                    ret.next_selected_preset = Some(i as u32);
                } else {
                    println!("Warning: value of `default_preset` is outside presets range");
                }
            } else {
                println!("Warning: value of `default_preset` is invalid");
            }
        }
        
        if let Some(val) = config_table.get("highlight_colors") {
            // Snippet by YgorSouza at https://github.com/emilk/egui/issues/3466#issuecomment-1762923933
            fn color_from_hex(hex: &str) -> Option<Color32> {
                let hex = hex.trim_start_matches('#');
                let alpha = match hex.len() {
                    6 => false,
                    8 => true,
                    _ => None?,
                };
                u32::from_str_radix(hex, 16)
                    .ok()
                    .map(|u| if alpha { u } else { u << 8 | 0xff })
                    .map(u32::to_be_bytes)
                    .map(|[r, g, b, a]| Color32::from_rgba_unmultiplied(r, g, b, a))
            }
            let a = val.as_array().unwrap();
            for ii in 0..8 {
                ret.highlight_colors[ii] = color_from_hex(a[ii].as_str().unwrap()).unwrap();
            }
        };
        
        if let Some(val) = config_table.get("show_timer_miliseconds") {
            ret.show_timer_miliseconds = val.as_bool().unwrap();
        }
        if let Some(val) = config_table.get("show_delta") {
            ret.show_delta = val.as_bool().unwrap();
        }
        if let Some(val) = config_table.get("show_neighbors") {
            ret.show_neighbors = val.as_bool().unwrap();
        }
        if let Some(val) = config_table.get("unlimited_zoom") {
            ret.unlimited_zoom = val.as_bool().unwrap();
        }
        if let Some(val) = config_table.get("probe_marked") {
            ret.probe_marked = val.as_bool().unwrap();
        }
        
        if let Some(val) = config_table.get("tile_spacings") {
            let a = val.as_array().unwrap();
            for ii in 0..4 {
                let valf = a[ii].as_float().unwrap() as f32;
                if valf >= 0.0 {
                    ret.tile_spacings[ii] = valf;
                }
            }
        }
        
        ret
    }

    fn reset(&mut self) {
        self.game = None;
        self.cursor_mode = CursorMode::ProbeAndMark;
    }

    fn start(&mut self, initial: [usize; DIMENSIONS_COUNT]) {
        self.start_time = Some(SystemTime::now());
        self.end_time = None;
        if let Some(seed) = &self.current_initial_settings.seed {
            self.game = Some(GameBoard::new(self.current_initial_settings.size,
                                            self.current_initial_settings.wrap,
                                            self.current_initial_settings.mines,
                                            None,
                                            u64::from_str_radix(&seed, 16).ok()));
            self.game.as_mut().unwrap().probe_at(initial, true);
        } else {
            self.game = Some(GameBoard::new(self.current_initial_settings.size,
                                            self.current_initial_settings.wrap,
                                            self.current_initial_settings.mines,
                                            Some(initial),
                                            None));
        }
    }

    // Translate and Scale from screen coordinates to cell coordinates
    // Uses modular cutoff to decide in constant time whether mouse is over any cell:
    //
    //   If current position modulo (x*cell+(x-1)*space_1+space_2)
    //     is larger than (x*cell+(x-1)*space_1), it must be outside a cell, otherwise repeat:
    //   |      |         |      |         |      |         |      |         |
    //   | cell | space_1 | cell | space_1 | cell | space_1 | cell | space_2 |
    //   |      |         |      |         |      |         |      |         |
    fn get_coords(&self, pos: Pos2) -> Option<[usize; DIMENSIONS_COUNT]> {
        if pos.x < self.view_origin.x || pos.y < self.view_origin.y {
            return None;
        }
        
        let [c_xx, c_yy, c_zz, c_uu, c_vv, c_ww] = self.current_initial_settings.size;
        let [sp_xx, sp_yy, sp_zz, sp_uu, sp_vv, sp_ww] = self.tile_spacings;
        
        // Sizes of blocks (cells + inner spacing)
        let x_block_size = c_xx as f32 * self.cell_edge + (c_xx - 1) as f32 * sp_xx;
        let y_block_size = c_yy as f32 * self.cell_edge + (c_yy - 1) as f32 * sp_yy;
        let z_block_size = c_zz as f32 * x_block_size + (c_zz - 1) as f32 * sp_zz;
        let u_block_size = c_uu as f32 * y_block_size + (c_uu - 1) as f32 * sp_uu;
        // let v_block_size = c_vv as f32 * z_block_size + (c_vv - 1) as f32 * sp_vv;
        // let w_block_size = c_ww as f32 * u_block_size + (c_ww - 1) as f32 * sp_ww;
        
        // Get logical points
        let (dx, dy) = ((pos.x - self.view_origin.x) / self.zoom_factor,
                        (pos.y - self.view_origin.y) / self.zoom_factor);
        
        // Modulo largest period
        let (dx_m1, dy_m1) = (dx as f32 % (z_block_size + sp_vv),
                              dy as f32 % (u_block_size + sp_ww));
        if dx_m1 > z_block_size || dy_m1 > u_block_size {
            return None;
        }
        // Modulo middle period
        let (dx_m2, dy_m2) = (dx_m1 % (x_block_size + sp_zz),
                              dy_m1 % (y_block_size + sp_uu));
        if dx_m2 > x_block_size || dy_m2 > y_block_size {
            return None;
        }
        // Modulo smallest period
        let (dx_m3, dy_m3) = (dx_m2 % (self.cell_edge + sp_xx),
                              dy_m2 % (self.cell_edge + sp_yy));
        if dx_m3 > self.cell_edge || dy_m3 > self.cell_edge {
            return None;
        }
        
        // Is cell-like, get coords
        let (xx, yy) = ((dx_m2 / (self.cell_edge + sp_xx)) as usize,
                        (dy_m2 / (self.cell_edge + sp_yy)) as usize);
        let (zz, uu) = ((dx_m1 / (x_block_size + sp_zz)) as usize,
                        (dy_m1 / (y_block_size + sp_uu)) as usize);
        let (vv, ww) = ((dx / (z_block_size + sp_vv)) as usize,
                        (dy / (u_block_size + sp_ww)) as usize);
        
        // Check if actual cell within grid bounds
        if xx >= c_xx || yy >= c_yy || zz >= c_zz || uu >= c_uu || vv >= c_vv || ww >= c_ww {
            return None;
        }
        
        return Some([xx, yy, zz, uu, vv, ww]);
    }
    
    // Scale and Translate from logical points to screen coordinates
    fn sc_tr(&self, xx: f32, yy: f32) -> Pos2 {
        Pos2::new(xx * self.zoom_factor, yy * self.zoom_factor) + self.view_origin.to_vec2()
    }
    
    // Cumulative spacing in screen axis directions
    fn cum_spc_x(&self, xx: usize, zz: usize, vv: usize) -> f32 {
        let [c_xx, _, _, _, _, _] = self.current_initial_settings.size;
        let [sp_xx, _, sp_zz, _, sp_vv, _] = self.tile_spacings;
        return (xx+zz*(c_xx-1)+vv*(c_xx-1)) as f32*sp_xx + (zz+vv*(c_xx-1)) as f32*sp_zz + vv as f32*sp_vv;
    }
    fn cum_spc_y(&self, yy: usize, uu: usize, ww: usize) -> f32 {
        let [_, c_yy, _, _, _, _] = self.current_initial_settings.size;
        let [_, sp_yy, _, sp_uu, _, sp_ww] = self.tile_spacings;
        return (yy+uu*(c_yy-1)+ww*(c_yy-1)) as f32*sp_yy + (uu+ww*(c_yy-1)) as f32*sp_uu + ww as f32*sp_ww;
    }
    
    fn try_set_cursor(&mut self, mode: CursorMode) {
        match mode {
            CursorMode::ProbeAndMark => {
                self.cursor_mode = CursorMode::ProbeAndMark;
            },
            CursorMode::Highlighter => {
                if self.game != None {
                    self.cursor_mode = CursorMode::Highlighter;
                }
            }
        }
    }
    
    fn reset_view(&mut self) {
        self.view_origin = Pos2::new(0.0, 20.0);
        self.zoom_factor = 1.0;
    }
    
    fn zoom_to_fit(&mut self, screen_size: Pos2) {
        let [c_xx, c_yy, c_zz, c_uu, c_vv, c_ww] = self.current_initial_settings.size;
        let [sp_xx, sp_yy, sp_zz, sp_uu, sp_vv, sp_ww] = self.tile_spacings;
        
        // TODO: allow user to set the padding
        let (padding_x, padding_y) = (5.0, 5.0);
        
        let x_block_size = c_xx as f32 * self.cell_edge + (c_xx - 1) as f32 * sp_xx;
        let y_block_size = c_yy as f32 * self.cell_edge + (c_yy - 1) as f32 * sp_yy;
        let z_block_size = c_zz as f32 * x_block_size + (c_zz - 1) as f32 * sp_zz;
        let u_block_size = c_uu as f32 * y_block_size + (c_uu - 1) as f32 * sp_uu;
        let v_block_size = c_vv as f32 * z_block_size + (c_vv - 1) as f32 * sp_vv;
        let w_block_size = c_ww as f32 * u_block_size + (c_ww - 1) as f32 * sp_ww;
        
        let x_factor = (screen_size.x - 2.0*padding_x) / v_block_size;
        let y_factor = (screen_size.y - 40.0 - 2.0*padding_y) / w_block_size;
        
        // Zoom to fit the larger side
        if (x_factor > y_factor && w_block_size * x_factor <= screen_size.y - 40.0 - 2.0*padding_y)
           || v_block_size * y_factor > screen_size.x - 2.0*padding_x {
            self.zoom_factor = if self.unlimited_zoom {x_factor} else {x_factor.clamp(0.01, 5.0)};
        } else {
            self.zoom_factor = if self.unlimited_zoom {y_factor} else {y_factor.clamp(0.01, 5.0)};
        }
        
        // Translate to center
        self.view_origin.x = (screen_size.x - 10.0 - v_block_size*self.zoom_factor) / 2.0 + padding_x;
        self.view_origin.y = (screen_size.y - 50.0 - w_block_size*self.zoom_factor) / 2.0 + 20.0 + padding_y;
    }
}

impl eframe::App for MinesweeperViewController {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.show_timer_miliseconds {
            ctx.request_repaint();
        } else {
            // TODO: This egui function is bugged, uncomment next line when fixed
            //ctx.request_repaint_after(Duration::new(1,0));
        }
        
        let mut new_game_window_enabled = self.new_game_window_enabled;
        if new_game_window_enabled {
            egui::Window::new("New Custom Game")
                .open(&mut new_game_window_enabled).show(ctx, |ui| {
                
                let resp = egui::ComboBox::from_id_source("preset_combobox")
                            .width(300.0)
                            .selected_text(if let Some(pno) = self.next_selected_preset {
                                self.presets[pno as usize].name.as_str()
                            } else {
                                "Custom game"
                            })
                            .show_ui(ui, |ui| {
                                (0..self.presets.len()).map(|ii| {
                                    ui.selectable_value(&mut self.next_selected_preset, Some(ii as u32),
                                        self.presets[ii].name.clone())
                                }).collect::<Vec<_>>()
                            });
                if let Some(col) = resp.inner {
                    if col.iter().any(|o: &Response| o.clicked()) {
                        if let Some(pno) = self.next_selected_preset {
                            self.next_initial_settings = self.presets[pno as usize].clone();
                        }
                    }
                }
                
                egui::Grid::new("dim_and_wrap_grid").show(ui, |ui| {
                    ui.label("Dimensions: ");
                    let resps = (0..DIMENSIONS_COUNT).map(
                        |e| ui.add(egui::DragValue::new(&mut self.next_initial_settings.size[e])
                                    .speed(1).clamp_range(1..=100))
                    ).collect::<Vec<_>>();
                    if resps.iter().any(|e| e.changed()) {
                        self.next_selected_preset = None;
                    };
                    ui.end_row();
                    
                    ui.label("Wrapping: ");
                    let resps = (0..DIMENSIONS_COUNT).map(
                        |e| ui.add(egui::Checkbox::without_text(&mut self.next_initial_settings.wrap[e]))
                    ).collect::<Vec<_>>();
                    if resps.iter().any(|e| e.changed()) {
                        self.next_selected_preset = None;
                    };
                    ui.end_row();
                });
                
                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.label("Mines: ");
                    ui.add(egui::DragValue::new(&mut self.next_initial_settings.mines).speed(1)
                        .clamp_range(1..=(self.next_initial_settings.size.iter().fold(1, |p, v| p*v)-1)));
                });
                
                let mut checkbox_state = self.next_initial_settings.seed != None;
                let checkbox = egui::Checkbox::new(&mut checkbox_state, "Generate the board based on the first click");
                if ui.add(checkbox).clicked() {
                    if self.next_initial_settings.seed == None {
                        self.next_initial_settings.seed = Some("".into());
                    } else {
                        self.next_initial_settings.seed = None;
                    }
                }
                
                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    ui.label("Seed: ");
                    
                    if let Some(ref mut s) = self.next_initial_settings.seed {
                        ui.add_enabled(true, egui::TextEdit::singleline(&mut *s));
                    } else {
                        ui.add_enabled(false, egui::TextEdit::singleline(&mut ""));
                    };
                });
                
                ui.with_layout(egui::Layout::left_to_right(egui::Align::TOP), |ui| {
                    if ui.button("Reset").clicked() {
                        self.next_initial_settings = self.current_initial_settings.clone();
                    }
                    if ui.button("Start").clicked() {
                        self.current_initial_settings = self.next_initial_settings.clone();
                        self.new_game_window_enabled = false;
                        self.reset();
                    }
                });
            });
        }
        self.new_game_window_enabled = self.new_game_window_enabled && new_game_window_enabled;
        
        let mut rules_window_enabled = self.rules_window_enabled;
        if rules_window_enabled {
            egui::Window::new("Rules")
                .open(&mut rules_window_enabled).show(ctx, |ui| {
               ui.label(
r"Rules are basically the same as with old-school minsweeper.

Cell's number signifies how many mines are in its neighborhood, 0/empty meaning none. Probing a cell containing mine results in game over, goal of the game is to uncover (through probing) all cells not containing mines. Fields suspected of being mines may be marked with a flag, however it is not necessary.

The twist is that in n dimensions, every cell has up to 3^n-1 neighbors (e.g. 8 for 2 dimensions, 26 for 3 dimensions, 80 for 4 dimensions)");
            });
        }
        self.rules_window_enabled = rules_window_enabled;
        let mut controls_window_enabled = self.controls_window_enabled;
        if controls_window_enabled {
            egui::Window::new("Controls")
                .open(&mut controls_window_enabled).show(ctx, |ui| {
               ui.label(
r"Currently there are two tools: Probe/Mark and Highlighter.

Probe/Mark probes a cell with primary button (usually Left Mouse Button) and marks a cell as a mine with secondary button (usually Right Mouse Button)

Highlighter highlights with primary button and unhighlights with secondary button.

Camera may be panned through dragging with middle mouse button, zoomed/unzoomed using scroll wheel.

If neighbor hints are enabled, holding Shift freezes them in place, whereas holding Alt temporarily disables them.");
            });
        }
        self.controls_window_enabled = controls_window_enabled;
        let mut about_window_enabled = self.about_window_enabled;
        if about_window_enabled {
            egui::Window::new("About")
                .open(&mut about_window_enabled).show(ctx, |ui| {
               ui.label(format!(
r"Minesweeper4D (version {})

Code written by sdasda7777 (github.com/sdasda7777) (except where noted otherwise) with a lot of help from amazing members of the egui Discord server", option_env!("CARGO_PKG_VERSION").unwrap()));
            });
        }
        self.about_window_enabled = about_window_enabled;
        
        TopBottomPanel::top("menubar_panel")
            .frame(egui::Frame::none().fill(egui::Color32::LIGHT_BLUE))
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(egui::Color32::BLACK);
                menu::bar(ui, |ui| {
                    ui.menu_button("Game", |ui| {
                        let new_game_window_button = Button::new("New Custom Game")
                                                        .selected(self.new_game_window_enabled);
                        if ui.add(new_game_window_button).clicked() {
                            self.new_game_window_enabled = !self.new_game_window_enabled;
                            ui.close_menu();
                        }
                        if ui.button("Quick restart").clicked() {
                            self.reset();
                            ui.close_menu();
                        }
                    });
                    ui.menu_button("View", |ui| {
                        let _ = ui.button(format!("Current zoom: {:.3} %", self.zoom_factor*100.0));
                        let reset_view_button = Button::new("Reset to 0x0 @ 100%")
                            .shortcut_text(
                                RichText::new(ctx.format_shortcut(&self.shortcuts.reset_view_shortcut))
                                    .color(Color32::WHITE));
                        if ui.add(reset_view_button).clicked() {
                            self.reset_view();
                            ui.close_menu();
                        }
                        let zoom_to_fit_button = Button::new("Zoom to fit")
                            .shortcut_text(
                                RichText::new(ctx.format_shortcut(&self.shortcuts.zoom_to_fit_shortcut))
                                    .color(Color32::WHITE));
                        if ui.add(zoom_to_fit_button).clicked() {
                            self.zoom_to_fit(ctx.screen_rect().max);
                            ui.close_menu();
                        }
                        let show_neighbors_button = Button::new("Show neighbors")
                                                    .selected(self.show_neighbors);
                        if ui.add(show_neighbors_button).clicked() {
                            self.show_neighbors = !self.show_neighbors;
                            ui.close_menu();
                        }
                        let unlimited_zoom_button = Button::new("Unlimited zoom")
                                                    .selected(self.unlimited_zoom);
                        if ui.add(unlimited_zoom_button).clicked() {
                            self.unlimited_zoom = !self.unlimited_zoom;
                            ui.close_menu();
                        }
                        let show_timer_miliseconds_button = Button::new("Show timer miliseconds")
                                                    .selected(self.show_timer_miliseconds);
                        if ui.add(show_timer_miliseconds_button).clicked() {
                            self.show_timer_miliseconds = !self.show_timer_miliseconds;
                            ui.close_menu();
                        }
                    });
                    ui.menu_button("Tools", |ui| {
                        ui.visuals_mut().widgets.noninteractive.weak_bg_fill = Color32::DARK_GRAY;
                        
                        let probe_and_mark_button = Button::new("Probe/Mark")
                            .selected(self.cursor_mode == CursorMode::ProbeAndMark)
                            .shortcut_text(
                                RichText::new(ctx.format_shortcut(&self.shortcuts.probe_mark_shortcut))
                                    .color(Color32::WHITE));
                        
                        let highlight_button = Button::new("Highlighter")
                             .selected(if self.cursor_mode == CursorMode::Highlighter {true} else {false})
                             .shortcut_text(
                                RichText::new(ctx.format_shortcut(&self.shortcuts.highlighter_shortcut))
                                    .color(Color32::WHITE));
                        
                        if ui.add(probe_and_mark_button).clicked() {
                            self.try_set_cursor(CursorMode::ProbeAndMark);
                            ui.close_menu();
                        }
                        ui.menu_button("Probe/Mark options", |ui| {
                            let probe_marked = Button::new("Allow probing marked cells").selected(self.probe_marked);
                            if ui.add(probe_marked).clicked() {
                                self.probe_marked = !self.probe_marked;
                            }
                        });
                        
                        // The highligh tool is disabled when game is NotRunning
                        if ui.add_enabled(self.game != None, highlight_button).clicked() {
                            self.try_set_cursor(CursorMode::Highlighter);
                            ui.close_menu();
                        }
                        ui.menu_button("Highlight groups", |ui| {
                            for ii in 0..8 {
                                let highlight_group_button
                                    = Button::new(format!("Group {} ({})", ii+1,
                                               if (self.selected_highlighters & (1 << ii)) > 0 {"on"} else {"off"}))
                                        .selected((self.selected_highlighters & (1 << ii)) > 0)
                                        .stroke(Stroke::new(2.0, self.highlight_colors[ii]))
                                        .shortcut_text(ctx.format_shortcut(&self.shortcuts.highlight_group_shortcuts[ii]));
                                
                                if ui.add(highlight_group_button).clicked() {
                                    self.selected_highlighters ^= 1 << ii;
                                }
                            }
                        });
                    });
                    ui.menu_button("Help", |ui| {
                        let rules_button = Button::new("Rules").selected(self.rules_window_enabled);
                        let controls_button = Button::new("Controls").selected(self.controls_window_enabled);
                        let about_button = Button::new("About").selected(self.about_window_enabled);
                        if ui.add(rules_button).clicked() {
                            self.rules_window_enabled = !self.rules_window_enabled;
                            ui.close_menu();
                        }
                        if ui.add(controls_button).clicked() {
                            self.controls_window_enabled = !self.controls_window_enabled;
                            ui.close_menu();
                        }
                        if ui.add(about_button).clicked() {
                            self.about_window_enabled = !self.about_window_enabled;
                            ui.close_menu();
                        }
                    });
                    if ui.button(format!("Δ: {}", if self.show_delta {"yes"} else {"no"})).clicked() {
                        self.show_delta = !self.show_delta;
                    }
                    if let Some(game) = &self.game {
                        let seed = format!("seed: {:016x}", game.seed());
                        ui.add(egui::TextEdit::singleline(&mut seed.as_str()));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        ui.button(format!("({}/{})  {}",
                                  if let Some(game) = &self.game {game.marked_as_mine()} else {0},
                                  self.current_initial_settings.mines,
                                  (0..DIMENSIONS_COUNT).map(
                                    |i| format!("{}{}",
                                        self.current_initial_settings.size[i],
                                        if self.current_initial_settings.wrap[i] {"w"} else {""})
                                  ).join(" x ")))
                    });
                });
            });
        
        TopBottomPanel::bottom("bottom_panel")
            .frame(egui::Frame::none().fill(egui::Color32::LIGHT_BLUE))
            .show(ctx, |ui| {
                ui.visuals_mut().override_text_color = Some(egui::Color32::BLACK);
                menu::bar(ui, |ui| {
                    match self.cursor_mode {
                        CursorMode::ProbeAndMark => {
                            let _ = ui.button("Probe/Mark: primary to probe a cell, secondary to mark as a mine");
                        },
                        CursorMode::Highlighter => {
                            let _ = ui.button(
                                format!("Highlighter ({}): primary to highlight a cell, secondary to unhighlight", 
                                    self.selected_highlighters)
                            );
                        }
                    }
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
                        if let Some(game) = &self.game {
                            if let Some(start_time) = self.start_time {
                                let dur = if let Some(end_time) = self.end_time
                                            { end_time } else { SystemTime::now() }
                                          .duration_since(start_time).unwrap();
                                let fdur = if self.show_timer_miliseconds
                                            { dur.hhmmssxxx() } else { dur.hhmmss() };
                                let _ = ui.button(format!("{}  {}",match game.state() {
                                                                    GameState::Victory => "You won!",
                                                                    GameState::Loss => "You lost!",
                                                                    _ => ""
                                                                 }, fdur
                                ));
                            }
                        }
                    });
                });
            });
        
        egui::CentralPanel::default()
        .frame(egui::Frame::none().fill(egui::Color32::GRAY))
        .show(ctx, |ui| {

            let basic_stroke = Stroke::new(2.0 * self.zoom_factor, egui::Color32::BLACK);
            let harder_stroke = Stroke::new(4.0 * self.zoom_factor, egui::Color32::BLACK);
            let neighbor_stroke = Stroke::new(3.0 * self.zoom_factor, self.neighbor_color);
            let center_stroke = Stroke::new(3.0 * self.zoom_factor, self.center_color);
            let selection_stroke = Stroke::new(3.0 * self.zoom_factor, self.selection_color);
            let highlight_strokes = self.highlight_colors.map(|x| Stroke::new(2.0 * self.zoom_factor, x));

            let screen_size = ctx.screen_rect().max;
            let [c_xx, c_yy, c_zz, c_uu, c_vv, c_ww] = self.current_initial_settings.size;
            
            let (painter_response, painter) = ui.allocate_painter(ui.available_size(), Sense::click_and_drag());

            // Paint cell contents
            let background_color = Color32::GRAY;
            if self.zoom_factor > 0.05 {
                if let Some(game) = &self.game {
                    for iw in 0..c_ww {
                    for iv in 0..c_vv {
                    for iu in 0..c_uu {
                    for iz in 0..c_zz {
                    for iy in 0..c_yy {
                    for ix in 0..c_xx {
                        let (spc_x, spc_y) = (self.cum_spc_x(ix,iz,iv), self.cum_spc_y(iy,iu,iw));
                        let ulc = self.sc_tr((ix+iz*c_xx+iv*c_xx*c_zz) as f32 * self.cell_edge + spc_x,
                                             (iy+iu*c_yy+iw*c_yy*c_uu) as f32 * self.cell_edge + spc_y);
                        // Only draw symbols reasonably close to the viewport
                        if ulc.x >= -self.cell_edge*self.zoom_factor && ulc.x <= screen_size.x
                           && ulc.y >= -self.cell_edge*self.zoom_factor && ulc.y <= screen_size.y {
                            let (symbol, color) = match game.cell_at([ix, iy, iz, iu, iv, iw]) {
                                CellState::UndiscoveredMine(_)
                                    => if game.state() == GameState::Victory {
                                            ("💣".into(), Color32::GREEN)
                                       } else if game.state() == GameState::Loss {
                                            ("💣".into(), Color32::RED)
                                       } else {
                                            ("".into(), Color32::GRAY)
                                       },
                                CellState::MarkedMine(_)
                                    => if game.state() == GameState::Victory || game.state() == GameState::Loss {
                                            ("🚩".into(), Color32::GREEN)
                                       } else {
                                            ("🚩".into(), Color32::GRAY)
                                       },
                                CellState::ExplodedMine(_) => ("💥".into(), Color32::RED),
                                CellState::UndiscoveredEmpty(..) => ("".into(), Color32::GRAY),
                                CellState::MarkedEmpty(..)
                                    => if game.state() == GameState::Victory || game.state() == GameState::Loss {
                                            ("🚩".into(), Color32::RED)
                                       } else {
                                            ("🚩".into(), Color32::GRAY)
                                       },
                                CellState::DiscoveredEmpty(mc, delta, _)
                                    => (if mc == 0 && delta == 0 {"".into()}
                                        else {format!("{}", if self.show_delta {delta} else {mc as i32})},
                                        Color32::LIGHT_GRAY),
                            };
                            
                            // Only paint squares with different color than the current background
                            if color != background_color {
                                painter.add(
                                    Shape::rect_filled(
                                        Rect::from_min_max(
                                                ulc,
                                                self.sc_tr((ix+iz*c_xx+iv*c_xx*c_zz+1) as f32 * self.cell_edge + spc_x,
                                                           (iy+iu*c_yy+iw*c_yy*c_uu+1) as f32 * self.cell_edge + spc_y)),
                                        Rounding::ZERO, color
                                    )
                                );
                            }
                            
                            if symbol != "" {
                                // Since drawing text is somewhat expensive, only draw text that can most definitely be read
                                if self.zoom_factor >= 0.10 {
                                    painter.text(
                                        self.sc_tr((ix+iz*c_xx+iv*c_xx*c_zz) as f32 * self.cell_edge+self.cell_edge/2.0 + spc_x,
                                                   (iy+iu*c_yy+iw*c_yy*c_uu) as f32 * self.cell_edge+self.cell_edge/2.0 + spc_y),
                                        Align2::CENTER_CENTER,
                                        symbol,
                                        FontId::proportional(25.0 * self.zoom_factor),
                                        Color32::BLACK
                                    );
                                } else {
                                    painter.add(
                                        Shape::circle_filled(
                                            self.sc_tr((ix+iz*c_xx+iv*c_xx*c_zz) as f32 * self.cell_edge+self.cell_edge/2.0 + spc_x,
                                                       (iy+iu*c_yy+iw*c_yy*c_uu) as f32 * self.cell_edge+self.cell_edge/2.0 + spc_y),
                                            10.0 * self.zoom_factor,
                                            Color32::GRAY)
                                    );
                                }
                            }
                        }
                    }}}}}}
                }
            }
            
            // Paint lines
            for iw in 0..c_ww {
            for iu in 0..c_uu {
            for iy in 0..c_yy {
                let spc_y = self.cum_spc_y(iy,iu,iw);
                let pos_y = (iy+iu*c_yy+iw*c_yy*c_uu) as f32 * self.cell_edge + spc_y;
                for iv in 0..c_vv {
                for iz in 0..c_zz {
                for ix in 0..c_xx {
                for ix2 in 0..=1 {
                    let spc_x = self.cum_spc_x(ix,iz,iv);
                    let ulc = self.sc_tr((ix+iz*c_xx+iv*c_xx*c_zz+ix2) as f32 * self.cell_edge + spc_x, pos_y);
                    if ulc.x >= -self.cell_edge*self.zoom_factor && ulc.x <= screen_size.x
                       && ulc.y >= -self.cell_edge*self.zoom_factor && ulc.y <= screen_size.y+self.cell_edge {
                        painter.add(
                            Shape::line_segment(
                                [ulc,
                                self.sc_tr((ix+iz*c_xx+iv*c_xx*c_zz+ix2) as f32 * self.cell_edge + spc_x,
                                            pos_y + self.cell_edge)],
                                if (ix == 0 && ix2 == 0)
                                    || (ix+1 == c_xx && ix2 == 1)
                                {harder_stroke} else {basic_stroke}));
                    }
                }}}}
            }}}
            for iv in 0..c_vv {
            for iz in 0..c_zz {
            for ix in 0..c_xx {
                let spc_x = self.cum_spc_x(ix,iz,iv);
                let pos_x = (ix+iz*c_xx+iv*c_xx*c_zz) as f32 * self.cell_edge + spc_x;
                for iw in 0..c_ww {
                for iu in 0..c_uu {
                for iy in 0..c_yy {
                for iy2 in 0..=1 {
                    let spc_y = self.cum_spc_y(iy,iu,iw);
                    let ulc = self.sc_tr(pos_x, (iy+iu*c_yy+iw*c_yy*c_uu+iy2) as f32*self.cell_edge + spc_y);
                    if ulc.x >= -self.cell_edge*self.zoom_factor && ulc.x <= screen_size.x
                       && ulc.y >= -self.cell_edge*self.zoom_factor && ulc.y <= screen_size.y {
                        painter.add(
                            Shape::line_segment(
                                [ulc,
                                 self.sc_tr(pos_x + self.cell_edge,
                                            (iy+iu*c_yy+iw*c_yy*c_uu+iy2) as f32*self.cell_edge + spc_y)],
                                if (iy == 0 && iy2 == 0)
                                   || (iy+1 == c_yy && iy2 == 1)
                                {harder_stroke} else {basic_stroke}));
                    }
                }}}}
            }}}
            
            // Paint cursor, neighbor hints and their center
            if let Some(pos) = painter_response.hover_pos() {
                let mut neighbor_coords = None;
                let mut mouse_coords = None;
                
                if self.show_neighbors && !ui.input(|i| i.modifiers.matches(Modifiers::ALT)) {
                    if ui.input(|i| i.modifiers.matches(Modifiers::SHIFT)) && self.neighbor_coords != None {
                        neighbor_coords = self.neighbor_coords;
                    }
                }
                if let Some(coords) = self.get_coords(pos) {
                    if neighbor_coords == None && self.show_neighbors
                       && !ui.input(|i| i.modifiers.matches(Modifiers::ALT)) {
                        self.neighbor_coords = Some(coords);
                        neighbor_coords = Some(coords);
                    }
                    mouse_coords = Some(coords);
                }
                
                if let Some([ix, iy, iz, iu, iv, iw]) = neighbor_coords {
                    let [cw_xx, cw_yy, cw_zz, cw_uu, cw_vv, cw_ww] = self.current_initial_settings.wrap;
                    for iwsupp in BWI::new(iw as i32-1,iw as i32+1,0,c_ww as i32-1,cw_ww) {
                    for ivsupp in BWI::new(iv as i32-1,iv as i32+1,0,c_vv as i32-1,cw_vv) {
                    for iusupp in BWI::new(iu as i32-1,iu as i32+1,0,c_uu as i32-1,cw_uu) {
                    for izsupp in BWI::new(iz as i32-1,iz as i32+1,0,c_zz as i32-1,cw_zz) {
                    for iysupp in BWI::new(iy as i32-1,iy as i32+1,0,c_yy as i32-1,cw_yy) {
                    for ixsupp in BWI::new(ix as i32-1,ix as i32+1,0,c_xx as i32-1,cw_xx) {
                        let (ixb, iyb, izb, iub, ivb, iwb)
                            = (ixsupp as usize, iysupp as usize, izsupp as usize,
                               iusupp as usize, ivsupp as usize, iwsupp as usize);
                        let (spc_x, spc_y) = (self.cum_spc_x(ixb,izb,ivb), self.cum_spc_y(iyb,iub,iwb));
                        let (ulc_x, ulc_y) = (ixb+izb*c_xx+ivb*c_xx*c_zz, iyb+iub*c_yy+iwb*c_yy*c_uu);
                        painter.add(
                            Shape::rect_stroke(
                                Rect::from_min_max(
                                    self.sc_tr(ulc_x as f32 * self.cell_edge + spc_x,
                                               ulc_y as f32 * self.cell_edge + spc_y),
                                    self.sc_tr((ulc_x+1) as f32 * self.cell_edge + spc_x,
                                               (ulc_y+1) as f32 * self.cell_edge + spc_y)),
                                    Rounding::ZERO, neighbor_stroke));
                    }}}}}}
                    
                    let (spc_x, spc_y) = (self.cum_spc_x(ix,iz,iv), self.cum_spc_y(iy,iu,iw));
                    let (ulc_x, ulc_y) = (ix+iz*c_xx+iv*c_xx*c_zz, iy+iu*c_yy+iw*c_yy*c_uu);
                    painter.add(
                        Shape::rect_stroke(
                            Rect::from_min_max(
                                self.sc_tr(ulc_x as f32 * self.cell_edge + spc_x,
                                           ulc_y as f32 * self.cell_edge + spc_y),
                                self.sc_tr((ulc_x+1) as f32 * self.cell_edge + spc_x,
                                           (ulc_y+1) as f32 * self.cell_edge + spc_y)),
                            Rounding::ZERO, center_stroke));
                }
                if let Some([ix, iy, iz, iu, iv, iw]) = mouse_coords {
                    let (spc_x, spc_y) = (self.cum_spc_x(ix,iz,iv), self.cum_spc_y(iy,iu,iw));
                    let (ulc_x, ulc_y) = (ix+iz*c_xx+iv*c_xx*c_zz, iy+iu*c_yy+iw*c_yy*c_uu);
                    painter.add(
                        Shape::rect_stroke(
                            Rect::from_min_max(
                                self.sc_tr(ulc_x as f32 * self.cell_edge + spc_x,
                                           ulc_y as f32 * self.cell_edge + spc_y),
                                self.sc_tr((ulc_x+1) as f32 * self.cell_edge + spc_x,
                                           (ulc_y+1) as f32 * self.cell_edge + spc_y)),
                            Rounding::ZERO, selection_stroke));
                }
            }
            
            // Paint highlights
            const HIGHLIGHT_SPACING: f32 = 2.5;
            for iw in 0..c_ww {
            for iv in 0..c_vv {
            for iu in 0..c_uu {
            for iz in 0..c_zz {
            for iy in 0..c_yy {
            for ix in 0..c_xx {
                if let Some(game) = &self.game {
                    match game.cell_at([ix, iy, iz, iu, iv, iw]) {
                        CellState::UndiscoveredMine(g) | CellState::MarkedMine(g)
                        | CellState::ExplodedMine(g) | CellState::UndiscoveredEmpty(.., g)
                        | CellState::MarkedEmpty(.., g) | CellState::DiscoveredEmpty(.., g)
                            => {
                            let (spc_x, spc_y) = (self.cum_spc_x(ix,iz,iv), self.cum_spc_y(iy,iu,iw));
                            let mut next_start_group = 0;
                            if g > 0 { for current_side in 0..8 {
                                for highlight_group in (next_start_group..8).chain(0..next_start_group) {
                                    if (g & (1 << highlight_group)) > 0 {
                                        let mut p1x = (ix+iz*c_xx+iv*c_xx*c_zz) as f32 * self.cell_edge + spc_x;
                                        let mut p1y = (iy+iu*c_yy+iw*c_yy*c_uu) as f32 * self.cell_edge + spc_y;
                                        let mut p2x = (ix+iz*c_xx+iv*c_xx*c_zz) as f32 * self.cell_edge + spc_x;
                                        let mut p2y = (iy+iu*c_yy+iw*c_yy*c_uu) as f32 * self.cell_edge + spc_y;
                                        match current_side {
                                            0 | 6 | 7 => {p1x += HIGHLIGHT_SPACING;},
                                            1 | 5 => {p1x += self.cell_edge/2.0;},
                                            2 | 3 | 4 => {p1x += self.cell_edge-HIGHLIGHT_SPACING;},
                                            _ => {}
                                        };
                                        match current_side {
                                            0 | 1 | 2 => {p1y += HIGHLIGHT_SPACING;},
                                            3 | 7 => {p1y += self.cell_edge/2.0;},
                                            4 | 5 | 6 => {p1y += self.cell_edge-HIGHLIGHT_SPACING;},
                                            _ => {}
                                        };
                                        match current_side {
                                            0 | 4 => {p2x += self.cell_edge/2.0;},
                                            1 | 2 | 3 => {p2x += self.cell_edge-HIGHLIGHT_SPACING;},
                                            5 | 6 | 7 => {p2x += HIGHLIGHT_SPACING;},
                                            _ => {}
                                        };
                                        match current_side {
                                            0 | 1 | 7 => {p2y += HIGHLIGHT_SPACING;},
                                            2 | 6 => {p2y += self.cell_edge/2.0;},
                                            3 | 4 | 5 => {p2y += self.cell_edge-HIGHLIGHT_SPACING;},
                                            _ => {}
                                        };
                                        
                                        painter.add(
                                            Shape::line_segment([self.sc_tr(p1x, p1y),
                                                                 self.sc_tr(p2x, p2y)],
                                                                highlight_strokes[highlight_group]));
                                        next_start_group = (highlight_group + 1) % 8;
                                        break;
                                    }
                                }
                            }}
                        }
                    };
                }
            }}}}}}
            
            // React to clicks
            // TODO: Maybe polymorphism/enum impl wouldn't be a bad idea here
            if painter_response.clicked_by(PointerButton::Primary) {
                // println!("primary click");
                if CursorMode::ProbeAndMark == self.cursor_mode {
                    if let Some(pos) = ctx.pointer_interact_pos() {
                        if let Some(coords) = self.get_coords(pos) {
                            if let Some(game) = &mut self.game {
                                if game.state() != GameState::Victory && game.state() != GameState::Loss {
                                    match game.probe_at(coords, self.probe_marked) {
                                        GameState::Victory | GameState::Loss => {
                                            self.end_time = Some(SystemTime::now());
                                        },
                                        GameState::Running => {}
                                    }
                                }
                            } else {
                                self.start(coords);
                            }
                        }
                    }
                } else if self.cursor_mode == CursorMode::Highlighter {
                    if let Some(pos) = ctx.pointer_interact_pos() {
                        if let Some(coords) = self.get_coords(pos) {
                            if let Some(game) = &mut self.game {
                                game.highlight_at(coords, self.selected_highlighters, true);
                            }
                        }
                    }
                }
            }
            if painter_response.clicked_by(PointerButton::Secondary) {
                // println!("secondary click");
                if CursorMode::ProbeAndMark == self.cursor_mode {
                    if let Some(pos) = ctx.pointer_interact_pos() {
                        if let Some(coords) = self.get_coords(pos) {
                            if let Some(game) = &mut self.game {
                                if game.state() != GameState::Victory && game.state() != GameState::Loss {
                                    game.mark_at(coords);
                                }
                            }
                        }
                    }
                } else if self.cursor_mode == CursorMode::Highlighter {
                    if let Some(pos) = ctx.pointer_interact_pos() {
                        if let Some(coords) = self.get_coords(pos) {
                            if let Some(game) = &mut self.game {
                                game.highlight_at(coords, self.selected_highlighters, false);
                            }
                        }
                    }
                }
            }
            if painter_response.dragged() {
                if ui.input(|i| i.pointer.button_down(PointerButton::Middle)) {
                    //println!("dragged");
                    self.view_origin += painter_response.drag_delta();
                } else if ui.input(|i| i.pointer.button_down(PointerButton::Primary)) {
                    if self.cursor_mode == CursorMode::Highlighter {
                        if let Some(pos) = ctx.pointer_interact_pos() {
                            if let Some(coords) = self.get_coords(pos) {
                                if let Some(game) = &mut self.game {
                                    game.highlight_at(coords, self.selected_highlighters, true);
                                }
                            }
                        }
                    }
                } else if ui.input(|i| i.pointer.button_down(PointerButton::Secondary)) {
                    if self.cursor_mode == CursorMode::Highlighter {
                        if let Some(pos) = ctx.pointer_interact_pos() {
                            if let Some(coords) = self.get_coords(pos) {
                                if let Some(game) = &mut self.game {
                                    game.highlight_at(coords, self.selected_highlighters, false);
                                }
                            }
                        }
                    }
                }
            }
            // Zoom/unzoom
            if painter_response.hovered() {
                let delta = ctx.input(|i| i.scroll_delta);
                //println!("{:?}", delta.y);
                if delta.y > 0.0 && (self.zoom_factor < 5.0 || self.unlimited_zoom) {
                    if let Some(pos) = ctx.pointer_interact_pos() {
                        let old_factor = self.zoom_factor;
                        self.zoom_factor *= 1.5;
                        self.view_origin.x -= ((pos.x - self.view_origin.x) / old_factor) * (self.zoom_factor - old_factor);
                        self.view_origin.y -= ((pos.y - self.view_origin.y) / old_factor) * (self.zoom_factor - old_factor);
                    }
                } else if delta.y < 0.0 && (self.zoom_factor > 0.01 || self.unlimited_zoom) {
                    if let Some(pos) = ctx.pointer_interact_pos() {
                        let old_factor = self.zoom_factor;
                        self.zoom_factor /= 1.5;
                        self.view_origin.x -= ((pos.x - self.view_origin.x) / old_factor) * (self.zoom_factor - old_factor);
                        self.view_origin.y -= ((pos.y - self.view_origin.y) / old_factor) * (self.zoom_factor - old_factor);
                    }
                }
            }
            // Keyboard Shortcuts
            //   The check below is to prevent triggering when trying to type
            //     the seed in the new game window. It's a bit crude, but it works.
            if !self.new_game_window_enabled {
                // TODO: `consume_shortcut` instead of `key_pressed` would allow for more flexibility,
                // but `consume_shortcut` doesn't allow indeterminate states for modifiers (at least currently)
                if ui.input_mut(|i| i.key_pressed(self.shortcuts.probe_mark_shortcut.key)) {
                    self.try_set_cursor(CursorMode::ProbeAndMark);
                }
                if ui.input_mut(|i| i.key_pressed(self.shortcuts.highlighter_shortcut.key)) {
                    self.try_set_cursor(CursorMode::Highlighter);
                }
                for ii in 0..8 {
                    if ui.input_mut(|i| i.key_pressed(self.shortcuts.highlight_group_shortcuts[ii].key)) {
                        self.selected_highlighters ^= 1 << ii;
                    }
                }
                
                if ui.input_mut(|i| i.key_pressed(self.shortcuts.reset_view_shortcut.key)) {
                    self.reset_view();
                }
                if ui.input_mut(|i| i.key_pressed(self.shortcuts.zoom_to_fit_shortcut.key)) {
                    self.zoom_to_fit(ctx.screen_rect().max);
                }
            }
        });
    }
}
