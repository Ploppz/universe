use geometry::vec::Vec2;
use geometry::ray::Ray;
use world::World;
use net::Socket;
use net::msg::Message;
use world::color::Color;
use input::PlayerInput;
use err::*;
use tile_net::Collable;

use num_traits::Float;

use std::net::SocketAddr;
use std::vec::Vec;
use std::collections::HashMap;
use std::cmp::min;
use std::thread;
use std::time::Duration;

const WORLD_SIZE: usize = 700;

const ACCELERATION: f32 = 0.35;

pub struct Server {
    world: World,
    players: HashMap<SocketAddr, PlayerData>,

    // Networking
    socket: Socket,
}

// Thoughts
// How to store inputs for each player?
// And apply the inputs

impl Server {
    pub fn new() -> Server {
        let size = WORLD_SIZE as f32;
        let world = World::new(WORLD_SIZE, WORLD_SIZE, Vec2::new(size/4.0, size/2.0), Vec2::new(3.0*size/4.0, size/2.0), true);

        Server {
            world: world,
            players: HashMap::new(),

            socket: Socket::new(9123).unwrap(),
        }
    }
    pub fn run(&mut self) -> Result<()> {
        loop {
            let players = self.players.clone(); // TODO: Unnecessary clone?

            // Handle input
            for player in players.values() {
                self.handle_input(player.input, player.nr);
            }

            // Networking
            self.socket.update()?;

            // Receive messages
            let mut messages = Vec::new();
            for msg in self.socket.messages() {
                let msg = msg.chain_err(|| "Error in received message.")?;
                messages.push(msg);
            }
            for msg in messages {
                self.handle_message(msg.0, msg.1).chain_err(|| "Error in handling message.")?;
            }
            // Send messages
            let message = Message::PlayerPos (self.world.players.iter().map(|p| p.shape.pos).collect());
            self.broadcast(&message).chain_err(|| "Could not broadcast.")?;

            // Logic
            prof!["Logic", self.world.update()];
            thread::sleep(Duration::from_millis(16));
        }

    }

    fn broadcast(&mut self, msg: &Message) -> Result<()> {
        for client in self.players.keys() {
            self.socket.send_to(msg.clone(), *client)?;
        }
        Ok(())
    }
    fn broadcast_reliably(&mut self, msg: &Message) -> Result<()> {
        for client in self.players.keys() {
            self.socket.send_reliably_to(msg.clone(), *client)?;
        }
        Ok(())
    }

    fn collide_bullet(&mut self, player_nr: usize, pos: Vec2, direction: Vec2) {
        let value = if let Color::White = self.world.players[player_nr].shape.color { 0 } else { 255 };
        let mut ray = Ray::new(pos, direction);
        let mut state = ray.new_state(Color::White);
        ray.solve(&self.world.tilenet, &mut state);
        match state.hit_tile {
            Some(index) => {
                self.world.tilenet.set(&value, (index.0 as usize, index.1 as usize));
            },
            None => {
                // TODO delete bullet
            }
        }
    }

    fn handle_message(&mut self, src: SocketAddr, msg: Message) -> Result<()> {
        // Will ignore packets from unregistered connections
        match msg {
            Message::Join => self.new_connection(src)?,
            Message::Input (input) => {
                if let Some(ref mut player_data) = self.players.get_mut(&src) {
                    player_data.input = input;
                }
            },
            Message::ToggleGravity => self.world.gravity_on = !self.world.gravity_on,
            Message::BulletFire { pos, direction } => {
                let player_nr = self.players.get(&src).map(|x| x.nr);
                if let Some(player_nr) = player_nr {
                    self.collide_bullet(player_nr, pos, direction);
                }
            },
            _ => {}
        }
        Ok(())
    }

    fn handle_input(&mut self, input: PlayerInput, player_nr: usize) {
        if input.left {
            self.world.players[player_nr].accelerate(Vec2::new(-ACCELERATION, 0.0));
        }
        if input.right {
            self.world.players[player_nr].accelerate(Vec2::new(ACCELERATION, 0.0));

        }
        if input.up {
            if self.world.gravity_on {
                self.world.players[player_nr].jump();
            } else {
                self.world.players[player_nr].accelerate(Vec2::new(0.0, ACCELERATION));
            }
        }
        if input.down {
            if !self.world.gravity_on {
                self.world.players[player_nr].accelerate(Vec2::new(0.0, -ACCELERATION));
            }
        }
        /*
        if input.key_toggled_down(KeyCode::G) {
            self.gravity_on = ! self.gravity_on;
        }
        */
    }

    fn new_connection(&mut self, src: SocketAddr) -> Result<()> {
        info!("New connection!");
        // Add new player
        let (w_count, b_count) = self.world.count_player_colors();
        let color = if w_count >= b_count { Color::Black } else { Color::White };
        let player_nr = self.world.add_new_player(color);
        let _ = self.players.insert(src, PlayerData::new(player_nr));
        // Tell about the world size and other meta data
        self.socket.send_to(
            Message::Welcome {
                width: self.world.get_width(),
                height: self.world.get_height(),
                you_index: player_nr,
                players: self.world.players.iter().map(|x| x.shape.color).collect(),
                white_base: self.world.white_base,
                black_base: self.world.black_base,
            },
            src).chain_err(|| "Could not send Welcome packet.")?;

        // Send it the whole world
        // We will need to split it up because of limited package size
        let dim = Server::packet_dim(Socket::max_packet_size());
        let blocks = (self.world.get_width() / dim.0 + 1, self.world.get_height() / dim.1 + 1);
        for x in 0..blocks.0 {
            for y in 0..blocks.1 {
                self.send_world_rect(x * dim.0, y * dim.0, dim.0, dim.1, src)?;
                // thread::sleep(Duration::from_millis(15));
            }
        }
        self.broadcast_reliably(&Message::NewPlayer {nr: player_nr as u32, color: color})
            .chain_err(|| "Could not broadcast_reliably.")?;

        Ok(())
    }

    fn send_world_rect(&mut self, x: usize, y: usize, w: usize, h: usize, dest: SocketAddr) -> Result<()> {
        let w = min(x + w, self.world.tilenet.get_size().0) - x;
        let h = min(y + h, self.world.tilenet.get_size().1) - y;

        let pixels: Vec<u8> = self.world.tilenet.view_box((x, x+w, y, y+h)).map(|x| *x.0).collect();
        assert!(pixels.len() == w*h);
        let msg = Message::WorldRect { x: x, y: y, width: w, height: h, pixels: pixels};
        self.socket.send_reliably_to(msg, dest)?;
        Ok(())
    }

    /// ASSUMPTION: packet size is 2^n
    fn packet_dim(packet_size: usize) -> (usize, usize) {
        let n = (packet_size as f32).log(2.0).floor();
        (2.0.powf((n/2.0).ceil()) as usize, 2.0.powf((n/2.0).floor()) as usize)
    }
}

#[derive(Copy, Clone)]
struct PlayerData {
    input: PlayerInput,
    nr: usize,
}
impl PlayerData {
    pub fn new(nr: usize) -> PlayerData {
        PlayerData {
            input: PlayerInput::default(),
            nr: nr,
        }
    }
}
