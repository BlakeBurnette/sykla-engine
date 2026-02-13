use godot::prelude::*;

mod ftms;
mod projection;
mod ride_engine;
mod route;
mod world_generator;

struct SyklaExtension;

#[gdextension]
unsafe impl ExtensionLibrary for SyklaExtension {}
