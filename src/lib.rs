pub mod crypto;
pub mod server;
pub mod tests;
pub mod user;

#[macro_export]
macro_rules! routes {
    ( $init: expr, $($k: expr => $v: ident),* ) => {{
        let mut map = std::collections::HashMap::new();
        $({
            log::debug!("Constructing handler for discriminant: {}", $k as u8);
            let value: Box<dyn arke::server::command::CommandHandler> = Box::new($v::new($init));
            map.insert($k(Default::default()).discriminant(), value);
        })*
        map
    }};
}
