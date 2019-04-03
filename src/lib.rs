#[macro_use]
extern crate error_chain;
#[macro_use]
extern crate log;
#[macro_use]
extern crate serde;

error_chain!();

mod server;

pub use crate::server::{InvokeRequestBody, InvokeResponseBody};