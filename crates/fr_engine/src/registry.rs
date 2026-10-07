//! The component types a game registers: their schemas, which the validation of
//! scenes and the editor read, and the factories that construct the behaviours
//! implementing them.

use std::collections::HashMap;

use fr_document::{ComponentSchema, ComponentSource, SchemaSet, builtin_schemas};

use crate::behavior::Behavior;

/// Constructs a behaviour of one registered component type.
pub type BehaviorFactory = Box<dyn Fn() -> Box<dyn Behavior>>;

/// The component types of a game: the engine's built-in schemas, the game's own
/// schemas and the factory of each game type.
///
/// Registration is explicit: a game builds one registry in its own code and
/// hands it to the engine, so no behaviour registers itself as a side effect of
/// being linked.
pub struct ComponentRegistry {
    /// The schemas of every type.
    schemas: SchemaSet,
    /// The factory of each game type by key.
    factories: HashMap<String, BehaviorFactory>,
}

impl Default for ComponentRegistry {
    /// A registry holding the engine's built-in types and no behaviours.
    fn default() -> Self {
        Self::new()
    }
}

impl ComponentRegistry {
    /// Creates a registry holding the engine's built-in types.
    pub fn new() -> Self {
        Self {
            schemas: builtin_schemas(),
            factories: HashMap::new(),
        }
    }

    /// Registers a game component type implemented by a behaviour, replacing the
    /// schema and the factory of a type with the same key. The type is marked as
    /// the game's own.
    pub fn add_behavior(
        &mut self,
        mut schema: ComponentSchema,
        factory: impl Fn() -> Box<dyn Behavior> + 'static,
    ) {
        schema.source = ComponentSource::Game;
        self.factories.insert(schema.key.clone(), Box::new(factory));
        self.schemas.add(schema);
    }

    /// The schemas of every registered type, which documents are validated
    /// against.
    pub const fn schemas(&self) -> &SchemaSet {
        &self.schemas
    }

    /// The schema of a type.
    pub fn find(&self, key: &str) -> Option<&ComponentSchema> {
        self.schemas.find(key)
    }

    /// The factory of a type; none for a type no behaviour implements.
    pub fn factory(&self, key: &str) -> Option<&BehaviorFactory> {
        self.factories.get(key)
    }

    /// A short digest of the registered types that changes when any schema
    /// changes.
    pub fn fingerprint(&self) -> String {
        self.schemas.fingerprint()
    }
}
