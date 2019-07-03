use crate::glocals::*;
pub use failure::Error;
use fast_logger::{error, info, Logger};
use geometry::vec::Vec2;
use geometry::{boxit::Boxit, grid2d::Grid};
use laminar::Socket;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::{time::Instant, vec::Vec};
use vxdraw::{self, *};

pub mod client;
pub use client::*;
pub mod server;
pub use server::*;

pub type Id = u32;

pub struct Main {
    pub cli: Option<Client>,
    pub srv: Option<Server>,
}
impl Main {
    pub fn new(mut cli: Option<Client>, srv: Option<Server>, mut logger: Logger<Log>) -> Main {
        if let (Some(cli), Some(srv)) = (&mut cli, &srv) {
            if let Err(e) = cli.connect_to_server(srv.network.local_addr().unwrap()) {
                info![logger, "main", "Failed to connect to server"; "err" => e];
            }
        }
        Main { cli, srv }
    }
    pub fn entry_point(&mut self) {
        loop {
            if let Some(ref mut cli) = self.cli {
                cli.time = Instant::now();
                cli.tick_logic();
                if cli.logic.should_exit {
                    break;
                }
            }
            if let Some(ref mut srv) = self.srv {
                srv.time = Instant::now();
                srv.tick_logic();
            }
        }
    }
}

// ---

#[derive(Default, Debug, Clone)]
pub struct Bullet {
    pub direction: Vec2,
    pub position: Vec2,
    pub destruction: i32,
    // TODO: destruction, width and height are functions of the Weapon
    pub id: u32,
    pub ty: Weapon,
}

#[derive(Default, Debug, Serialize, Deserialize, Clone)]
pub struct PlayerData {
    pub position: Vec2,
    pub velocity: Vec2,
    pub id: Id,
    pub curr_weapon: Weapon,
    pub curr_weapon_cooldown: usize,
    /// Reality in which the player resides. Reality signifies the colour of the air in which the
    /// player resides.
    pub reality: u32,
}
impl PlayerData {
    pub fn new(id: Id, reality: u32, position: Vec2) -> PlayerData {
        PlayerData {
            position,
            velocity: Vec2::null_vec(),
            id,
            curr_weapon: Weapon::Hellfire,
            curr_weapon_cooldown: 0,
            reality,
        }
    }
}

#[derive(PartialEq, Debug, Serialize, Deserialize, Clone)]
pub enum Weapon {
    Hellfire,
    Ak47,
}

impl Default for Weapon {
    fn default() -> Self {
        Weapon::Hellfire
    }
}

#[derive(Copy, Clone)]
pub struct Vertex {
    pub pos: [f32; 2],
}

// Not sure where to put this. Helper for laminar::Socket
pub fn random_port_socket() -> Socket {
    let loopback = Ipv4Addr::new(127, 0, 0, 1);
    let socket = SocketAddrV4::new(loopback, 0);
    Socket::bind(socket).unwrap() // TODO laminar error not compatible with failure?
}

static FIREBALLS: &dyntex::ImgData =
    &dyntex::ImgData::PNGBytes(include_bytes!["../../assets/images/bullets.png"]);
static WEAPONS: &dyntex::ImgData =
    &dyntex::ImgData::PNGBytes(include_bytes!["../../assets/images/weapons.png"]);

pub fn initialize_grid(s: &mut Grid<(u8, u8, u8, u8)>) {
    s.resize(1000, 1000);
}

pub fn create_black_square_around_player(s: &mut Grid<(u8, u8, u8, u8)>) {
    for (i, j) in Boxit::with_center((100, 100), (500, 300)) {
        s.set(i, j, (0, 0, 0, 0));
    }
}

pub fn accelerate_player_according_to_input(
    inp: &UserInput,
    conf: &Config,
    on_ground: bool,
) -> Vec2 {
    let dy = if inp.is_down(InputKey::Up) {
        if conf.world.gravity_on {
            if on_ground {
                -conf.player.jump_acc
            } else {
                0.0
            }
        } else {
            -conf.player.acc
        }
    } else if inp.is_down(InputKey::Down) {
        conf.player.acc
    } else {
        0.0
    };
    let dx = if inp.is_down(InputKey::Left) {
        -conf.player.acc
    } else if inp.is_down(InputKey::Right) {
        conf.player.acc
    } else {
        0.0
    };
    Vec2 {
        x: dx as f32,
        y: dy as f32,
    } / if inp.is_down(InputKey::LShift) {
        3.0
    } else {
        1.0
    }
}

// TODO: split into client and server
/*
fn fire_bullets(
    s: &mut ClientLogic,
    graphics: &mut Option<Graphics>,
    random: &mut rand_pcg::Pcg64Mcg,
) {
    if s.input.is_left_mouse_button_down() {
        if s.current_weapon_cooldown == 0 {
            s.current_weapon_cooldown = match s.current_weapon {
                Weapon::Hellfire => 5,
                Weapon::Ak47 => 2,
            }
        } else {
            s.current_weapon_cooldown -= 1;
            return;
        }

        let weapon = &s.current_weapon;

        let spread = if weapon == &Weapon::Hellfire {
            0.3
        } else {
            0.1
        };

        let (
            width,
            height,
            animation_block_begin,
            animation_block_end,
            sprite_width,
            sprite_height,
            destruction,
            bullet_count,
            speed,
        ) = match weapon {
            Weapon::Hellfire => (10, 6, (0.0, 0.0), (1.0, 53.0 / 60.0), 6.8, 0.9, 3, 1, 1.0),
            Weapon::Ak47 => (
                1,
                1,
                (0.0, 54.0 / 60.0),
                (4.0 / 679.0, 58.0 / 60.0),
                0.5,
                0.5,
                1,
                1,
                2.0,
            ),
        };

        for _ in 0..bullet_count {
            let direction = if let Some(ref mut graphics) = graphics {
                (Vec2::from(s.input.get_mouse_pos())
                    - Vec2::from(graphics.windowing.get_window_size_in_pixels_float()) / 2.0)
                    .rotate(random.gen_range(-spread, spread))
            } else {
                Vec2 { x: 1.0, y: 0.0 }
            };

            let handle = if let Some(ref mut graphics) = graphics {
                Some(
                    graphics.windowing.dyntex().add(
                        &graphics.bullets_texture,
                        vxdraw::dyntex::Sprite::new()
                            .width(sprite_width)
                            .height(sprite_height)
                            .scale(3.0)
                            .origin((-sprite_width / 2.0, sprite_height / 2.0))
                            .rotation(Rad(-direction.angle() + std::f32::consts::PI)),
                    ),
                )
            } else {
                None
            };

            let position = s.players.get(0).map_or(Vec2 { x: 0.0, y: 0.0 }, |x| {
                x.position + Vec2 { x: 5.0, y: 5.0 }
            });
            s.bullets.push(ClientBullet {
                direction: direction.normalize() * speed,
                position,
                destruction,

                animation_sequence: 0,
                animation_block_begin,
                animation_block_end,
                height,
                width,
                current_uv_begin: (0.0, 0.0),
                current_uv_end: (0.0, 0.0),
                handle,
            });
        }
    }
}
*/

#[cfg(test)]
mod tests {
    #[test]
    fn basic_setup_and_teardown() {
        crate::game::Server::new(fast_logger::Logger::spawn_void());
    }
}
