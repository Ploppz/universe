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

    /// Physical response to collision - i.e. bounce in direction of normal
    pub fn collide_wall(&mut self, normal: Vec2) -> (Vec2, Vec2) {
        const RESTITUTION: f32 = 0.7;
        let normal = normal.normalize();
        let tangent = Vec2::new(-normal.y, normal.x);
        self.vel = tangent.scale(Vec2::dot(self.vel, tangent)) +
                   normal.scale(Vec2::dot(self.vel, normal).abs() * RESTITUTION);
        (tangent, tangent.scale(-1.0))
    }
}

#[derive(Default)]
pub struct PolygonState {

    /* Config / Algorithm */
    pub queued_vel: Vec2,
    current_try: usize,

    /* Results */
    pub collision: bool,
    pub poc: (i32, i32),    // point of collision
    pub toc: f32,           // time of collision - between this and next frame

    pub debug_vectors: Vec<(Vec2, Vec2)>,
}
impl PolygonState {
    /// start_toc is what fraction of the velocity we start the algorithm with
    pub fn new(start_toc: f32, vel: Vec2) -> PolygonState {
        let mut result = PolygonState::default();
        result.toc = start_toc;
        result.queued_vel = vel;
        result
    }
}
impl CollableState for PolygonState {
    fn queued(&self) -> Vector {
        (self.queued_vel * self.toc).into()
    }

}

impl Collable<u8, PolygonState> for Polygon {
    fn points(&self) -> Points {
        Points::new(Vector(self.pos.x, self.pos.y), &self.points)
    }

    // fn presolve(&mut self, state: &mut PolygonState) { }

    fn resolve<I>(&mut self, mut set: TileSet<Tile, I>, state: &mut PolygonState) -> bool
        where I: Iterator<Item = (i32, i32)>
    {
        if set.all(|x| *x == 0) {
            // If there is no collision (we only collide with non-zero tiles)
            self.pos += Vec2::from(state.queued());
            true
        } else {
            // Collision.

            info!(" - Collision!"; "vel.x" => self.vel.x, "vel.y" => self.vel.y);

            state.collision = true;
            state.poc = set.get_coords();
            state.toc *= 0.9;
            state.current_try += 1;

            false
        }
    }

    // fn postsolve(&mut self, _collided_once: bool, _resolved: bool, state: &mut PolygonState) { }
}
