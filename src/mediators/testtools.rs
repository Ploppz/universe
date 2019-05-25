use crate::glocals::*;
use std::net::TcpStream;

pub fn spawn_gameshell(s: &mut Main) {
    let game_shell = crate::mediators::game_shell::spawn_with_any_port(s.logger.clone());
    s.threads.game_shell = Some(game_shell.thread_handle);
    s.threads.game_shell_keep_running = Some(game_shell.keep_running);
    s.threads.game_shell_channel = Some(game_shell.channel);
    s.threads.game_shell_channel_send = Some(game_shell.channel_send);
    s.threads.game_shell_port = Some(game_shell.port);
    // std::thread::sleep(std::time::Duration::new(1, 0));
    s.threads.game_shell_connection =
        Some(TcpStream::connect("127.0.0.1:".to_string() + &game_shell.port.to_string()).unwrap());
}

pub fn gsh(s: &mut Main, input: &str) -> String {
    use std::io::{Read, Write};
    use std::str::from_utf8;
    let conn = s.threads.game_shell_connection.as_mut().unwrap();
    conn.write_all(input.as_bytes()).unwrap();
    conn.write_all(b"\n").unwrap();
    conn.flush().unwrap();

    let mut buffer = [0u8; 1024];
    let count = conn.read(&mut buffer).unwrap();

    from_utf8(&buffer[..count]).unwrap().to_string()
}

/// Runs a gsh command while also performing an operating between the write and read stages
///
/// Gsh runs in its own thread, meaning that for main to see some results, it needs to run a
/// function on main to access gsh data from some channel.
pub fn gsh_synchronous(s: &mut Main, input: &str, tween: fn(&mut Main)) -> String {
    use std::io::{Read, Write};
    use std::str::from_utf8;
    {
        assert![
            s.threads
                .game_shell_channel
                .as_mut()
                .unwrap()
                .try_recv()
                .is_err(),
            "Channel should be empty before sending a gsh command."
        ];
        let conn = s.threads.game_shell_connection.as_mut().unwrap();
        conn.write_all(input.as_bytes()).unwrap();
        conn.write_all(b"\n").unwrap();
        conn.flush().unwrap();
        let msg = s
            .threads
            .game_shell_channel
            .as_mut()
            .unwrap()
            .recv()
            .unwrap();
        s.threads
            .game_shell_channel_send
            .as_mut()
            .unwrap()
            .send(msg)
            .expect("Unable to requeue message");
    }

    tween(s);

    let mut buffer = [0u8; 1024];
    let count = s
        .threads
        .game_shell_connection
        .as_mut()
        .unwrap()
        .read(&mut buffer)
        .unwrap();

    from_utf8(&buffer[..count]).unwrap().to_string()
}