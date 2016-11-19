use tile_net::*;
use geometry::vec::Vec2;
use global::Tile;

pub struct Polygon {
    pub points: Vec<(f32, f32)>, // Vec<Vec2> later. Now: for convenience with TileNet
    pub pos: Vec2,
    pub ori: f32,

    pub vel: Vec2, // rot: f32,
}

impl Polygon {
    pub fn new_quad(start_x: f32, start_y: f32, width: f32, height: f32) -> Polygon {
        let mut result = Polygon {
            points: Vec::new(),
            pos: Vec2::new(start_x, start_y),
            ori: 0.0,
            vel: Vec2::null_vec(),
        };
        result.points.push((0.0, 0.0));
        result.points.push((0.0, height));
        result.points.push((width, height));
        result.points.push((width, 0.0));
        result
    }
}

impl Collable<u8, ()> for Polygon {
    fn points(&self) -> Points {
        Points::new(Vector(self.pos.x, self.pos.y), &self.points)
    }

    fn queued(&self) -> Vector {
        // Returns velocity vector (new name?)
        Vector(self.vel.x, self.vel.y)
    }

    fn resolve<I>(&mut self, mut set: TileSet<Tile, I>, _state: &mut ()) -> bool
        where I: Iterator<Item = (i32, i32)>
    {
        if set.all(|x| *x == 0) {
            // If there is no collision (we only collide with non-zero tiles)
            self.pos += self.vel;
            true
        } else if self.vel.length_squared() > 1e-6 {
            // There was collision, but our speed isn't tiny
            self.vel = self.vel * 0.9;
            false
        } else {
            // This may happen if we generate a world where we're stuck in a tile,
            // normally this will never happen, this library can preserve consistently
            // perfectly.
            self.vel = Vec2::new(0.0, 0.0);
            true
        }
    }
}
