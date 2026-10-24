//! Window-independent music application services.
pub mod api;
pub mod lyrics;
pub mod model;
pub mod persistence;
#[cfg(feature = "audio")]
pub mod player;
pub mod queue;

pub use api::Api;
pub use model::*;
pub use persistence::Paths;
#[cfg(feature = "audio")]
pub use player::Core;
