//! Stable value handles to GPU-side resources.

/// Defines a copyable, hashable handle type wrapping a `u32` index.
macro_rules! handle {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
        pub struct $name(u32);

        impl $name {
            /// Wraps `index`; only the owner of the resource table should call this.
            pub const fn from_index(index: u32) -> Self {
                Self(index)
            }

            /// The index this handle was created from.
            pub const fn index(self) -> usize {
                self.0 as usize
            }
        }
    };
}

handle! {
    /// A mesh uploaded to the renderer.
    MeshId
}

handle! {
    /// A material uploaded to the renderer.
    MaterialId
}

handle! {
    /// A texture uploaded to the renderer.
    TextureId
}
