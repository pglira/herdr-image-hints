//! The use cases: what herdr-image-hints does, expressed against ports the
//! adapters implement. No terminal, no socket, no process in here.

pub mod pick;
pub mod ports;
pub mod start;

#[cfg(test)]
pub(crate) mod testing;
