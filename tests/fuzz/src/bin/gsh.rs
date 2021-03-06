use gameshell::Evaluate;
use honggfuzz::fuzz;
use universe::mediators::game_shell::make_new_gameshell;

fn main() {
    let mut gsh = make_new_gameshell();
    loop {
        fuzz!(|data: &[u8]| {
            if let Ok(data) = std::str::from_utf8(data) {
                let _ = gsh.interpret_single(data);
                let _ = gsh.interpret_multiple(data);
            }
        });
    }
}
