use fast_logger::Logger;
use honggfuzz::fuzz;
use metac::Evaluate;
use universe::mediators::game_shell::make_new_gameshell;

fn main() {
    let logger = Logger::spawn_void();
    let mut gsh = make_new_gameshell(logger);
    loop {
        fuzz!(|data: &[u8]| {
            if let Ok(data) = std::str::from_utf8(data) {
                let _ = gsh.interpret_single(data);
                let _ = gsh.interpret_multiple(data);
            }
        });
    }
}