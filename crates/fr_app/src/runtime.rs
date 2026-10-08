//! The application builder, plugin contracts and standard execution stages.

pub use bevy_app::{
    App as Runtime, First, FixedFirst, FixedLast, FixedPostUpdate, FixedPreUpdate, FixedUpdate,
    Last, Plugin, Plugins, PostStartup, PostUpdate, PreStartup, PreUpdate, Startup, Update,
};
