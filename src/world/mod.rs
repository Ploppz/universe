pub mod gen;

use std::vec::Vec;

use glium::glutin::VirtualKeyCode;

use tile_net::TileNet;
use tile_net::Collable;

use global::Tile;
use geometry::polygon::Polygon;
use geometry::vec::Vec2;
use input::Input;

pub struct World {
    pub tilenet: TileNet<Tile>,
    pub polygons: Vec<Polygon>,
    pub exit: bool,
    width: usize,
    height: usize,
}

impl World {
    pub fn new(width: usize, height: usize) -> World {
        World {
            tilenet: TileNet::<Tile>::new(width, height),
            polygons: Vec::new(),
            exit: false,
            width: width,
            height: height,
        }
    }

    pub fn get_normal(&self, world_x: usize, world_y: usize) -> Vec2 {
        let kernel = [[-1.0, 0.0, 1.0], [-2.0, 0.0, 2.0], [-1.0, 0.0, 1.0]];
        let mut dx = 0.0;
        let mut dy = 0.0;
        for y in 0..3 {
            for x in 0..3 {
                // Change the unwraps here
                dx += kernel[y][x] *
                      (*self.tilenet.get((world_x + x - 1, world_y + y - 1)).unwrap() as f32);
                dy += kernel[x][y] *
                      (*self.tilenet.get((world_x + x - 1, world_y + y - 1)).unwrap() as f32);
            }
        }
        Vec2::new(dx, dy)
    }

    pub fn update(&mut self, input: &Input) {
        // Ad hoc: input to control first polygon
        if input.key_down(VirtualKeyCode::Escape) {
            self.exit = true;
        }
        if input.key_down(VirtualKeyCode::Left) || input.key_down(VirtualKeyCode::A) {
            self.polygons[0].vel.x -= 1.0;
        }
        if input.key_down(VirtualKeyCode::Right) || input.key_down(VirtualKeyCode::D) {
            self.polygons[0].vel.x += 1.0;
        }
        if input.key_down(VirtualKeyCode::Up) || input.key_down(VirtualKeyCode::W) {
            self.polygons[0].vel.y += 1.0;
        }
        if input.key_down(VirtualKeyCode::Down) || input.key_down(VirtualKeyCode::S) {
            self.polygons[0].vel.y -= 1.0;
        }

        for p in &mut self.polygons {
            // p.queued = p.vel;
            let mut i = 0;
            const MAX_ITER: i32 = 10;
            p.solve(&self.tilenet, &mut ());
        }
        // Friction
        for p in &mut self.polygons {
            p.vel = p.vel * 0.9;
        }
    }

    pub fn get_width(&self) -> usize {
        self.width
    }
    pub fn get_height(&self) -> usize {
        self.height
    }

    pub fn print(&self) {
        info!("TileNet"; "content" => format!["{:?}", self.tilenet]);
    }
}
