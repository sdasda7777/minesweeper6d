
extern crate rand;
use bwi::BWI;
use rand_chacha::{ChaCha8Rng, rand_core::SeedableRng};
use self::rand::Rng;
use std::collections::VecDeque;

pub const DIMENSIONS_COUNT: usize = 6;

#[derive(Clone,PartialEq)]
pub struct InitialGameSettings {
    pub name: String,
    pub size: [usize; DIMENSIONS_COUNT],
    pub wrap: [bool; DIMENSIONS_COUNT],
    pub mines: u32,
    pub seed: Option<String>,
}

impl Default for InitialGameSettings {
    fn default() -> Self {
        Self {
            name: "unnamed".into(),
            size: [4, 4, 4, 4, 1, 1],
            wrap: [false, false, false, false, false, false],
            mines: 20,
            seed: None,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum GameState {
    Running,
    Victory,
    Loss,
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub enum CellState {
    // Mine is either undiscoved, marked or exploded
    // u8 are highlight groups
    UndiscoveredMine(u8), // nothing (💣 if different one exploded)
    MarkedMine(u8),   // 🚩
    ExplodedMine(u8), // 💥
    
    // Empty fields are either undiscovered, marked or discovered
    // u32 is the true total value, i32 is the delta
    // u8 are highlight groups
    UndiscoveredEmpty(u32, i32, u8), // nothing
    MarkedEmpty(u32, i32, u8), // 🚩
    DiscoveredEmpty(u32, i32, u8), // u32 || i32 (when delta is enabled)
}

#[derive(Debug, PartialEq)]
pub struct GameBoard {
    // x, y, z, u, v, w
    size: [usize; DIMENSIONS_COUNT],
    wrap: [bool; DIMENSIONS_COUNT],
    
    seed: u64,
    
    // w, v, u, z, y, x
    board: Vec<Vec<Vec<Vec<Vec<Vec<CellState>>>>>>,
    
    state: GameState,
    mine_count: u32,
    marked_as_mine: u64,
    undiscoved_empty_fields: u64,
    total_fields: u64,
}

impl GameBoard {
    // Getters
    pub fn size(&self) -> [usize; DIMENSIONS_COUNT] {self.size}
    pub fn wrap(&self) -> [bool; DIMENSIONS_COUNT] {self.wrap}
    pub fn state(&self) -> GameState {self.state}
    pub fn seed(&self) -> u64 {self.seed}
    pub fn mines_present(&self) -> u32 {self.mine_count}
    pub fn marked_as_mine(&self) -> u64 {self.marked_as_mine}
    pub fn undiscoved_empty_fields(&self) -> u64 {self.undiscoved_empty_fields}
    pub fn total_fields(&self) -> u64 {self.total_fields}
    
    pub fn cell_at(&self, coordinates: [usize; DIMENSIONS_COUNT]) -> CellState {
        let [xx, yy, zz, uu, vv, ww] = coordinates;
        self.board[ww][vv][uu][zz][yy][xx]
    }
    
    // Used for discovering undiscovered fields
    pub fn probe_at(&mut self, coordinates: [usize; DIMENSIONS_COUNT], probe_marked: bool) -> GameState {
        let [xx, yy, zz, uu, vv, ww] = coordinates;
        let [s_x, s_y, s_z, s_u, s_v, s_w] = self.size;
        let [w_x, w_y, w_z, w_u, w_v, w_w] = self.wrap;
        
        let mut deque = VecDeque::from([(xx,yy,zz,uu,vv,ww)]);
        
        while let Some((ix, iy, iz, iu, iv, iw)) = deque.pop_front() {
            match self.board[iw][iv][iu][iz][iy][ix] {
                CellState::UndiscoveredMine(g) => {
                    self.board[iw][iv][iu][iz][iy][ix] = CellState::ExplodedMine(g);
                    self.state = GameState::Loss;
                },
                CellState::MarkedMine(g) => {
                    if probe_marked {
                        self.mark_at([ix,iy,iz,iu,iv,iw]); // Necessary to increase neighbor delta
                        self.board[iw][iv][iu][iz][iy][ix] = CellState::ExplodedMine(g);
                        self.state = GameState::Loss;
                    }
                },
                CellState::ExplodedMine(_) => {},
                CellState::UndiscoveredEmpty(c, d, g) => {
                    self.board[iw][iv][iu][iz][iy][ix] = CellState::DiscoveredEmpty(c, d, g);
                    self.undiscoved_empty_fields -= 1;
                    
                    // Recurse for cells with no mines in neighbors
                    if c == 0 {
                        for iwsupp in BWI::new(iw as i32-1,iw as i32+1,0,s_w as i32-1,w_w) {
                        for ivsupp in BWI::new(iv as i32-1,iv as i32+1,0,s_v as i32-1,w_v) {
                        for iusupp in BWI::new(iu as i32-1,iu as i32+1,0,s_u as i32-1,w_u) {
                        for izsupp in BWI::new(iz as i32-1,iz as i32+1,0,s_z as i32-1,w_z) {
                        for iysupp in BWI::new(iy as i32-1,iy as i32+1,0,s_y as i32-1,w_y) {
                        for ixsupp in BWI::new(ix as i32-1,ix as i32+1,0,s_x as i32-1,w_x) {
                            if ixsupp != ix as i32 || iysupp != iy as i32 || izsupp != iz as i32
                               || iusupp != iu as i32 || ivsupp != iv as i32 || iwsupp != iw as i32 {
                                // Coordinates might be added to the queue multiple times,
                                // but only undiscovered fields will produce more coordinates,
                                // so the search will terminate rather quickly.
                                deque.push_back((ixsupp as usize, iysupp as usize, izsupp as usize,
                                                 iusupp as usize, ivsupp as usize, iwsupp as usize));
                            }
                        }}}}}}
                    }
                },
                CellState::MarkedEmpty(c, d, g) => {
                    if probe_marked {
                        self.mark_at([ix,iy,iz,iu,iv,iw]); // Necessary to increase neighbor delta
                        self.board[iw][iv][iu][iz][iy][ix] = CellState::DiscoveredEmpty(c, d, g);
                        self.undiscoved_empty_fields -= 1;
                    }
                },
                CellState::DiscoveredEmpty(..) => {},
            }
        };
        if self.state != GameState::Loss && self.undiscoved_empty_fields == 0 {
            self.state = GameState::Victory;
        }
        return self.state;
    }
    
    // Used for marking/unmarking cells as mines
    pub fn mark_at(&mut self, coordinates: [usize; DIMENSIONS_COUNT]) {
        let [xx, yy, zz, uu, vv, ww] = coordinates;
        let [s_x, s_y, s_z, s_u, s_v, s_w] = self.size;
        let [w_x, w_y, w_z, w_u, w_v, w_w] = self.wrap;
        
        match self.board[ww][vv][uu][zz][yy][xx] {
            CellState::UndiscoveredMine(..) | CellState::UndiscoveredEmpty(..) => {
                /*subtract 1 from empty neighbors' delta*/
                for iwsupp in BWI::new(ww as i32-1,ww as i32+1,0,s_w as i32-1,w_w) {
                for ivsupp in BWI::new(vv as i32-1,vv as i32+1,0,s_v as i32-1,w_v) {
                for iusupp in BWI::new(uu as i32-1,uu as i32+1,0,s_u as i32-1,w_u) {
                for izsupp in BWI::new(zz as i32-1,zz as i32+1,0,s_z as i32-1,w_z) {
                for iysupp in BWI::new(yy as i32-1,yy as i32+1,0,s_y as i32-1,w_y) {
                for ixsupp in BWI::new(xx as i32-1,xx as i32+1,0,s_x as i32-1,w_x) {
                    if iwsupp != ww as i32 || ivsupp != vv as i32 || iusupp != uu as i32
                       || izsupp != zz as i32 || iysupp != yy as i32 || ixsupp != xx as i32 {
                        match self.board[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                        [izsupp as usize][iysupp as usize][ixsupp as usize] {
                            CellState::UndiscoveredEmpty(c, d, g) => {
                                self.board[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                          [izsupp as usize][iysupp as usize][ixsupp as usize]
                                    = CellState::UndiscoveredEmpty(c, d-1, g);
                            },
                            CellState::DiscoveredEmpty(c, d, g) => {
                                self.board[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                          [izsupp as usize][iysupp as usize][ixsupp as usize]
                                    = CellState::DiscoveredEmpty(c, d-1, g);
                            },
                            CellState::MarkedEmpty(c, d, g) => {
                                self.board[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                          [izsupp as usize][iysupp as usize][ixsupp as usize]
                                    = CellState::MarkedEmpty(c, d-1, g);
                            },
                            _ => {}
                        }
                    }
                }}}}}}
            },
            CellState::MarkedMine(..) | CellState::MarkedEmpty(..) => {
                /*add 1 to empty neighbors' delta*/
                for iwsupp in BWI::new(ww as i32-1,ww as i32+1,0,s_w as i32-1,w_w) {
                for ivsupp in BWI::new(vv as i32-1,vv as i32+1,0,s_v as i32-1,w_v) {
                for iusupp in BWI::new(uu as i32-1,uu as i32+1,0,s_u as i32-1,w_u) {
                for izsupp in BWI::new(zz as i32-1,zz as i32+1,0,s_z as i32-1,w_z) {
                for iysupp in BWI::new(yy as i32-1,yy as i32+1,0,s_y as i32-1,w_y) {
                for ixsupp in BWI::new(xx as i32-1,xx as i32+1,0,s_x as i32-1,w_x) {
                    if iwsupp != ww as i32 || ivsupp != vv as i32 || iusupp != uu as i32
                       || izsupp != zz as i32 || iysupp != yy as i32 || ixsupp != xx as i32 {
                        match self.board[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                        [izsupp as usize][iysupp as usize][ixsupp as usize] {
                            CellState::UndiscoveredEmpty(c, d, g) => {
                                self.board[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                          [izsupp as usize][iysupp as usize][ixsupp as usize]
                                    = CellState::UndiscoveredEmpty(c, d+1, g);
                            },
                            CellState::DiscoveredEmpty(c, d, g) => {
                                self.board[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                          [izsupp as usize][iysupp as usize][ixsupp as usize]
                                    = CellState::DiscoveredEmpty(c, d+1, g);
                            },
                            CellState::MarkedEmpty(c, d, g) => {
                                self.board[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                          [izsupp as usize][iysupp as usize][ixsupp as usize]
                                    = CellState::MarkedEmpty(c, d+1, g);
                            },
                            _ => {}
                        }
                    }
                }}}}}}
            },
            CellState::ExplodedMine(..) | CellState::DiscoveredEmpty(..) => {/*do nothing*/}
        };
        
        // Mark unmarked, unmark marked
        match self.board[ww][vv][uu][zz][yy][xx] {
            CellState::UndiscoveredMine(g) => {
                self.marked_as_mine += 1;
                self.board[ww][vv][uu][zz][yy][xx] = CellState::MarkedMine(g);
            },
            CellState::MarkedMine(g) => {
                self.marked_as_mine -= 1;
                self.board[ww][vv][uu][zz][yy][xx] = CellState::UndiscoveredMine(g)
            },
            CellState::ExplodedMine(_) => {},
            CellState::UndiscoveredEmpty(c, d, g) => {
                self.marked_as_mine += 1;
                self.board[ww][vv][uu][zz][yy][xx] = CellState::MarkedEmpty(c, d, g);
            },
            CellState::MarkedEmpty(c, d, g) => {
                self.marked_as_mine -= 1;
                self.board[ww][vv][uu][zz][yy][xx] = CellState::UndiscoveredEmpty(c, d, g);
            },
            CellState::DiscoveredEmpty(_, _, _) => {}
        };
    }
    
    // Highlight given cell (enable = highlight, !enable = unhighlight)
    pub fn highlight_at(&mut self, coordinates: [usize; DIMENSIONS_COUNT], group: u8, enable: bool) {
        let [xx, yy, zz, uu, vv, ww] = coordinates;
        if enable {
            self.board[ww][vv][uu][zz][yy][xx] = match self.board[ww][vv][uu][zz][yy][xx] {
                CellState::UndiscoveredMine(g) => CellState::UndiscoveredMine(g | group),
                CellState::MarkedMine(g) => CellState::MarkedMine(g | group),
                CellState::ExplodedMine(g) => CellState::ExplodedMine(g | group),
                CellState::UndiscoveredEmpty(c, d, g) => CellState::UndiscoveredEmpty(c, d, g | group),
                CellState::MarkedEmpty(c, d, g) => CellState::MarkedEmpty(c, d, g | group),
                CellState::DiscoveredEmpty(c, d, g) => CellState::DiscoveredEmpty(c, d, g | group)
            };
        } else {
            self.board[ww][vv][uu][zz][yy][xx] = match self.board[ww][vv][uu][zz][yy][xx] {
                CellState::UndiscoveredMine(g) => CellState::UndiscoveredMine(g & !group),
                CellState::MarkedMine(g) => CellState::MarkedMine(g & !group),
                CellState::ExplodedMine(g) => CellState::ExplodedMine(g & !group),
                CellState::UndiscoveredEmpty(c, d, g) => CellState::UndiscoveredEmpty(c, d, g & !group),
                CellState::MarkedEmpty(c, d, g) => CellState::MarkedEmpty(c, d, g & !group),
                CellState::DiscoveredEmpty(c, d, g) => CellState::DiscoveredEmpty(c, d, g & !group)
            };
        }
    }
    
    pub fn new(sizes: [usize; DIMENSIONS_COUNT], wraps: [bool; DIMENSIONS_COUNT], mine_count: u32,
               initial: Option<[usize; DIMENSIONS_COUNT]>, seed: Option<u64>) -> Self {
        
        let [size_x, size_y, size_z, size_u, size_v, size_w] = sizes;
        let [wrap_x, wrap_y, wrap_z, wrap_u, wrap_v, wrap_w] = wraps;
        
        let mut dumb_rng = rand::thread_rng();
        let mut final_chacha_seed: u64;
        
        
        let mut board_6d = Vec::new();
        loop {
            // Generate empty board
            for _ in 0..size_w {
                let mut board_5d = Vec::new();
            for _ in 0..size_v {
                let mut board_4d = Vec::new();
            for _ in 0..size_u {
                let mut board_3d = Vec::new();
            for _ in 0..size_z {
                let mut board_2d = Vec::new();
            for _ in 0..size_y {
                let mut board_1d = Vec::new();
            for _ in 0..size_x {
                board_1d.push(CellState::UndiscoveredEmpty(0, 0, 0));
            } board_2d.push(board_1d);
            } board_3d.push(board_2d);
            } board_4d.push(board_3d);
            } board_5d.push(board_4d);
            } board_6d.push(board_5d);
            }
            
            // Generate mines into field
            final_chacha_seed = seed.unwrap_or_else(|| dumb_rng.gen());
            let mut rng = ChaCha8Rng::seed_from_u64(final_chacha_seed);
            let mut mines_placed = 0;
            while mines_placed < mine_count {
                let ix = if size_x > 1 {rng.gen_range(0..size_x)} else {0usize};
                let iy = if size_y > 1 {rng.gen_range(0..size_y)} else {0usize};
                let iz = if size_z > 1 {rng.gen_range(0..size_z)} else {0usize};
                let iu = if size_u > 1 {rng.gen_range(0..size_u)} else {0usize};
                let iv = if size_v > 1 {rng.gen_range(0..size_v)} else {0usize};
                let iw = if size_w > 1 {rng.gen_range(0..size_w)} else {0usize};
                
                if board_6d[iw][iv][iu][iz][iy][ix] == CellState::UndiscoveredEmpty(0, 0, 0) {
                   board_6d[iw][iv][iu][iz][iy][ix] = CellState::UndiscoveredMine(0);
                   mines_placed += 1;
                }
            }
            
            // Count neighbors
            for iw in 0..size_w {
            for iv in 0..size_v {
            for iu in 0..size_u {
            for iz in 0..size_z {
            for iy in 0..size_y {
            for ix in 0..size_x {
                let mut neighbouring_mines = 0;
                for iwsupp in BWI::new(iw as i32-1,iw as i32+1,0,size_w as i32-1,wrap_w) {
                for ivsupp in BWI::new(iv as i32-1,iv as i32+1,0,size_v as i32-1,wrap_v) {
                for iusupp in BWI::new(iu as i32-1,iu as i32+1,0,size_u as i32-1,wrap_u) {
                for izsupp in BWI::new(iz as i32-1,iz as i32+1,0,size_z as i32-1,wrap_z) {
                for iysupp in BWI::new(iy as i32-1,iy as i32+1,0,size_y as i32-1,wrap_y) {
                for ixsupp in BWI::new(ix as i32-1,ix as i32+1,0,size_x as i32-1,wrap_x) {
                    match board_6d[iwsupp as usize][ivsupp as usize][iusupp as usize]
                                  [izsupp as usize][iysupp as usize][ixsupp as usize] {
                        CellState::UndiscoveredMine(_) => {
                            neighbouring_mines += 1;
                        },
                        _ => {}
                    }
                }}}}}}
                
                if board_6d[iw][iv][iu][iz][iy][ix] == CellState::UndiscoveredEmpty(0, 0, 0) {
                    board_6d[iw][iv][iu][iz][iy][ix]
                        = CellState::UndiscoveredEmpty(neighbouring_mines, neighbouring_mines as i32, 0);
                }
            }}}}}}
            
            // Stop board generation if seed was inputted
            if seed != None {break;}
            // Test if initial field is empty, select as probed, otherwise repeat
            if let Some([ix, iy, iz, iu, iv, iw]) = initial {
                if let CellState::UndiscoveredEmpty(..) = board_6d[iw][iv][iu][iz][iy][ix] {
                    break;
                }
            } else { break; }
            board_6d = Vec::new();
        }
        
        let mut ret = Self {
            size: sizes,
            wrap: wraps,
            
            seed: final_chacha_seed,
            board: board_6d,
            
            state: GameState::Running,
            mine_count: mine_count,
            marked_as_mine: 0,
            undiscoved_empty_fields:
                size_x as u64 * size_y as u64 * size_z as u64
                * size_u as u64 * size_v as u64 * size_w as u64
                - mine_count as u64,
            total_fields:
                size_x as u64 * size_y as u64 * size_z as u64
                * size_u as u64 * size_v as u64 * size_w as u64,
        };
        
        // This also sets the state to failure if seed was used
        if let Some(init_coords) = initial {
            ret.probe_at(init_coords, false);
        };
        
        ret
    }
}
