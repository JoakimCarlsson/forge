//! The physics subsystem: the rigid body, collider and ragdoll contributions,
//! the collider's wire shape and bounds, and the physics entries of the Create
//! menu. Nothing simulates in the editor: these components are inert data whose
//! shapes are drawn so they can be authored.

mod collider;
mod ragdoll;
mod register;
mod rigid_body;

pub use register::register;
