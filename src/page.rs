//! Messages between a game and the web page it runs in: a score going out,
//! a list coming back. In a terminal there is no page, so `send` drops the
//! message and `recv` has none. On the page's side, funkey.js hands them to
//! the page's script (`game.listen`, `game.send`).

/// Hand the page a message.
pub fn send(msg: &str) {
    #[cfg(target_arch = "wasm32")]
    crate::web::outbox_push(msg.to_string());
    #[cfg(not(target_arch = "wasm32"))]
    let _ = msg;
}

/// The next message from the page, if one has come.
pub fn recv() -> Option<String> {
    #[cfg(target_arch = "wasm32")]
    return crate::web::inbox_pop();
    #[cfg(not(target_arch = "wasm32"))]
    None
}

/// True in a web page. There the page keeps what a terminal game keeps on
/// disk.
pub fn web() -> bool {
    cfg!(target_arch = "wasm32")
}
