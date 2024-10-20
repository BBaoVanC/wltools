use wayland_client::Connection;
use snafu::prelude::*;

#[snafu::report]
fn main() -> Result<(), snafu::Whatever> {
    let server_conn = Connection::connect_to_env().whatever_context("error connecting to Wayland server")?;
    Ok(())
}
