//! Persistent islands: groups of bodies connected by touching contacts and joints, which wake and
//! sleep together.
//!
//! Linking a contact or joint merges the islands of its bodies; removing one only counts, and an
//! island that wants to sleep is split along its remaining constraints first.

use std::collections::BTreeMap;

use crate::body::BodyId;
use crate::contact::ContactId;
use crate::joint::JointId;
use crate::slot::Handle;
use crate::world::World;

/// Marks [`IslandId`].
#[derive(Clone, Copy, Debug)]
pub struct IslandTag;

/// A handle to an island of a world.
pub type IslandId = Handle<IslandTag>;

/// A group of bodies that move or sleep together.
#[derive(Clone, Debug, Default)]
pub(crate) struct Island {
    /// The bodies of the island.
    pub(crate) bodies: Vec<BodyId>,
    /// The touching contacts that link the bodies.
    pub(crate) contacts: Vec<ContactId>,
    /// The joints that link the bodies.
    pub(crate) joints: Vec<JointId>,
    /// How many constraints were removed since the island was last split.
    pub(crate) constraint_remove_count: usize,
    /// Whether the island is asleep.
    pub(crate) asleep: bool,
}

/// Finds the root of `node` in the union-find forest with path halving.
fn find_parent(parents: &mut [usize], mut node: usize) -> usize {
    while parents[node] != node {
        let grand_parent = parents[parents[node]];
        parents[node] = grand_parent;
        node = grand_parent;
    }
    node
}

/// Joins the sets of `node1` and `node2` by rank.
fn union(parents: &mut [usize], ranks: &mut [usize], node1: usize, node2: usize) {
    let root1 = find_parent(parents, node1);
    let root2 = find_parent(parents, node2);
    if root1 == root2 {
        return;
    }
    if ranks[root1] < ranks[root2] {
        parents[root1] = root2;
    } else if ranks[root1] > ranks[root2] {
        parents[root2] = root1;
    } else {
        parents[root2] = root1;
        ranks[root1] += 1;
    }
}

impl World {
    /// Creates the single body island of a body that is not static.
    pub(crate) fn create_island_for_body(&mut self, body_id: BodyId, asleep: bool) {
        let island_id = self.islands.insert(Island {
            bodies: vec![body_id],
            asleep,
            ..Island::default()
        });
        if let Some(body) = self.bodies.get_mut(body_id) {
            body.island = island_id;
            body.island_index = 0;
        }
    }

    /// Removes a body from its island, destroying the island when it was the only body.
    pub(crate) fn remove_body_from_island(&mut self, body_id: BodyId) {
        let Some(body) = self.bodies.get(body_id) else {
            return;
        };
        let island_id = body.island;
        let index = body.island_index;
        if island_id.is_null() {
            return;
        }
        let sole = self
            .islands
            .get(island_id)
            .is_none_or(|i| i.bodies.len() <= 1);
        if sole {
            self.destroy_island(island_id);
        } else {
            let moved = self.islands.get_mut(island_id).and_then(|island| {
                island.bodies.swap_remove(index);
                island.constraint_remove_count += 1;
                island.bodies.get(index).copied()
            });
            if let Some(moved_id) = moved
                && let Some(moved_body) = self.bodies.get_mut(moved_id)
            {
                moved_body.island_index = index;
            }
        }
        if let Some(body) = self.bodies.get_mut(body_id) {
            body.island = IslandId::NULL;
            body.island_index = usize::MAX;
        }
    }

    /// Frees an island that has no bodies left.
    fn destroy_island(&mut self, island_id: IslandId) {
        if self.split_island == Some(island_id) {
            self.split_island = None;
        }
        self.islands.remove(island_id);
    }

    /// Merges two islands into the larger one and returns it; either may be null.
    fn merge_islands(&mut self, island_a: IslandId, island_b: IslandId) -> IslandId {
        if island_a == island_b {
            return island_a;
        }
        if island_a.is_null() {
            return island_b;
        }
        if island_b.is_null() {
            return island_a;
        }
        let count_a = self.islands.get(island_a).map_or(0, |i| i.bodies.len());
        let count_b = self.islands.get(island_b).map_or(0, |i| i.bodies.len());
        let (big_id, small_id) = if count_a >= count_b {
            (island_a, island_b)
        } else {
            (island_b, island_a)
        };
        let Some(small) = self.islands.remove(small_id) else {
            return big_id;
        };
        if self.split_island == Some(small_id) {
            self.split_island = None;
        }
        let (body_start, contact_start, joint_start) = match self.islands.get(big_id) {
            Some(big) => (big.bodies.len(), big.contacts.len(), big.joints.len()),
            None => return big_id,
        };
        for (offset, &body_id) in small.bodies.iter().enumerate() {
            if let Some(body) = self.bodies.get_mut(body_id) {
                body.island = big_id;
                body.island_index = body_start + offset;
            }
        }
        for (offset, &contact_id) in small.contacts.iter().enumerate() {
            if let Some(contact) = self.contacts.get_mut(contact_id) {
                contact.island = big_id;
                contact.island_index = contact_start + offset;
            }
        }
        for (offset, &joint_id) in small.joints.iter().enumerate() {
            if let Some(joint) = self.joints.get_mut(joint_id) {
                joint.island = big_id;
                joint.island_index = joint_start + offset;
            }
        }
        if let Some(big) = self.islands.get_mut(big_id) {
            big.bodies.extend_from_slice(&small.bodies);
            big.contacts.extend_from_slice(&small.contacts);
            big.joints.extend_from_slice(&small.joints);
            big.constraint_remove_count += small.constraint_remove_count;
        }
        big_id
    }

    /// Wakes the sleeping island of whichever of the two bodies sleeps when the other is awake.
    fn wake_for_link(&mut self, body_a: BodyId, body_b: BodyId) {
        let (awake_a, asleep_a, island_a) = self
            .bodies
            .get(body_a)
            .map_or((false, false, IslandId::NULL), |b| {
                (b.is_awake(), b.asleep, b.island)
            });
        let (awake_b, asleep_b, island_b) = self
            .bodies
            .get(body_b)
            .map_or((false, false, IslandId::NULL), |b| {
                (b.is_awake(), b.asleep, b.island)
            });
        if awake_a && asleep_b {
            self.wake_island(island_b);
        }
        if awake_b && asleep_a {
            self.wake_island(island_a);
        }
    }

    /// Links a contact that started touching, merging the islands of its bodies.
    pub(crate) fn link_contact(&mut self, contact_id: ContactId) {
        let Some(contact) = self.contacts.get(contact_id) else {
            return;
        };
        let (body_a, body_b) = (contact.body_a, contact.body_b);
        self.wake_for_link(body_a, body_b);
        let island_a = self.bodies.get(body_a).map_or(IslandId::NULL, |b| b.island);
        let island_b = self.bodies.get(body_b).map_or(IslandId::NULL, |b| b.island);
        let island_id = self.merge_islands(island_a, island_b);
        let Some(island) = self.islands.get_mut(island_id) else {
            return;
        };
        let index = island.contacts.len();
        island.contacts.push(contact_id);
        if let Some(contact) = self.contacts.get_mut(contact_id) {
            contact.island = island_id;
            contact.island_index = index;
        }
    }

    /// Unlinks a contact that stopped touching.
    pub(crate) fn unlink_contact(&mut self, contact_id: ContactId) {
        let Some(contact) = self.contacts.get(contact_id) else {
            return;
        };
        let island_id = contact.island;
        let index = contact.island_index;
        if island_id.is_null() {
            return;
        }
        let moved = self.islands.get_mut(island_id).and_then(|island| {
            island.contacts.swap_remove(index);
            island.constraint_remove_count += 1;
            island.contacts.get(index).copied()
        });
        if let Some(moved_id) = moved
            && let Some(moved_contact) = self.contacts.get_mut(moved_id)
        {
            moved_contact.island_index = index;
        }
        if let Some(contact) = self.contacts.get_mut(contact_id) {
            contact.island = IslandId::NULL;
            contact.island_index = usize::MAX;
        }
    }

    /// Links a joint, merging the islands of its bodies.
    pub(crate) fn link_joint(&mut self, joint_id: JointId) {
        let Some(joint) = self.joints.get(joint_id) else {
            return;
        };
        let (body_a, body_b) = (joint.body_a, joint.body_b);
        self.wake_for_link(body_a, body_b);
        let island_a = self.bodies.get(body_a).map_or(IslandId::NULL, |b| b.island);
        let island_b = self.bodies.get(body_b).map_or(IslandId::NULL, |b| b.island);
        let island_id = self.merge_islands(island_a, island_b);
        let Some(island) = self.islands.get_mut(island_id) else {
            return;
        };
        let index = island.joints.len();
        island.joints.push(joint_id);
        if let Some(joint) = self.joints.get_mut(joint_id) {
            joint.island = island_id;
            joint.island_index = index;
        }
    }

    /// Unlinks a joint that is being destroyed.
    pub(crate) fn unlink_joint(&mut self, joint_id: JointId) {
        let Some(joint) = self.joints.get(joint_id) else {
            return;
        };
        let island_id = joint.island;
        let index = joint.island_index;
        if island_id.is_null() {
            return;
        }
        let moved = self.islands.get_mut(island_id).and_then(|island| {
            island.joints.swap_remove(index);
            island.constraint_remove_count += 1;
            island.joints.get(index).copied()
        });
        if let Some(moved_id) = moved
            && let Some(moved_joint) = self.joints.get_mut(moved_id)
        {
            moved_joint.island_index = index;
        }
        if let Some(joint) = self.joints.get_mut(joint_id) {
            joint.island = IslandId::NULL;
            joint.island_index = usize::MAX;
        }
    }

    /// Splits an island along its remaining constraints into connected components.
    pub(crate) fn split_island_by_id(&mut self, base_id: IslandId) {
        let Some(base) = self.islands.get(base_id) else {
            return;
        };
        if base.constraint_remove_count == 0 {
            return;
        }
        let body_ids = base.bodies.clone();
        let contact_ids = base.contacts.clone();
        let joint_ids = base.joints.clone();
        let count = body_ids.len();
        let mut parents: Vec<usize> = (0..count).collect();
        let mut ranks = vec![0; count];
        let body_index = |world: &Self, id: BodyId| -> Option<usize> {
            world
                .bodies
                .get(id)
                .filter(|b| b.island == base_id)
                .map(|b| b.island_index)
        };
        for &contact_id in &contact_ids {
            if let Some(contact) = self.contacts.get(contact_id)
                && let (Some(a), Some(b)) = (
                    body_index(self, contact.body_a),
                    body_index(self, contact.body_b),
                )
            {
                union(&mut parents, &mut ranks, a, b);
            }
        }
        for &joint_id in &joint_ids {
            if let Some(joint) = self.joints.get(joint_id)
                && let (Some(a), Some(b)) = (
                    body_index(self, joint.body_a),
                    body_index(self, joint.body_b),
                )
            {
                union(&mut parents, &mut ranks, a, b);
            }
        }
        let mut component_of_root: BTreeMap<usize, usize> = BTreeMap::new();
        let mut component_count = 0;
        for i in 0..count {
            let root = find_parent(&mut parents, i);
            component_of_root.entry(root).or_insert_with(|| {
                component_count += 1;
                component_count - 1
            });
        }
        if component_count == 1 {
            if let Some(base) = self.islands.get_mut(base_id) {
                base.constraint_remove_count = 0;
            }
            return;
        }
        let asleep = self.islands.get(base_id).is_some_and(|i| i.asleep);
        self.islands.remove(base_id);
        let island_ids: Vec<IslandId> = (0..component_count)
            .map(|_| {
                self.islands.insert(Island {
                    asleep,
                    ..Island::default()
                })
            })
            .collect();
        for (i, &body_id) in body_ids.iter().enumerate() {
            let root = find_parent(&mut parents, i);
            let island_id = island_ids[component_of_root[&root]];
            if let Some(island) = self.islands.get_mut(island_id) {
                let index = island.bodies.len();
                island.bodies.push(body_id);
                if let Some(body) = self.bodies.get_mut(body_id) {
                    body.island = island_id;
                    body.island_index = index;
                }
            }
        }
        for &contact_id in &contact_ids {
            let Some(contact) = self.contacts.get(contact_id) else {
                continue;
            };
            let target = self.target_island(contact.body_a, contact.body_b);
            if let Some(island) = self.islands.get_mut(target) {
                let index = island.contacts.len();
                island.contacts.push(contact_id);
                if let Some(contact) = self.contacts.get_mut(contact_id) {
                    contact.island = target;
                    contact.island_index = index;
                }
            }
        }
        for &joint_id in &joint_ids {
            let Some(joint) = self.joints.get(joint_id) else {
                continue;
            };
            let target = self.target_island(joint.body_a, joint.body_b);
            if let Some(island) = self.islands.get_mut(target) {
                let index = island.joints.len();
                island.joints.push(joint_id);
                if let Some(joint) = self.joints.get_mut(joint_id) {
                    joint.island = target;
                    joint.island_index = index;
                }
            }
        }
        if self.split_island == Some(base_id) {
            self.split_island = None;
        }
    }

    /// The island of the first of two bodies that has one.
    fn target_island(&self, body_a: BodyId, body_b: BodyId) -> IslandId {
        let island_a = self.bodies.get(body_a).map_or(IslandId::NULL, |b| b.island);
        if island_a.is_null() {
            self.bodies.get(body_b).map_or(IslandId::NULL, |b| b.island)
        } else {
            island_a
        }
    }

    /// Puts an island to sleep unless it still has to be split. Returns whether it sleeps.
    pub(crate) fn try_sleep_island(&mut self, island_id: IslandId) -> bool {
        let Some(island) = self.islands.get(island_id) else {
            return false;
        };
        if island.constraint_remove_count > 0 && island.bodies.len() > 1 {
            return false;
        }
        let bodies = island.bodies.clone();
        for body_id in bodies {
            if let Some(body) = self.bodies.get_mut(body_id) {
                body.asleep = true;
                body.linear_velocity = fr_math::Vec3::ZERO;
                body.angular_velocity = fr_math::Vec3::ZERO;
            }
        }
        if let Some(island) = self.islands.get_mut(island_id) {
            island.asleep = true;
        }
        if self.split_island == Some(island_id) {
            self.split_island = None;
        }
        true
    }

    /// Wakes a sleeping island and resets the sleep timers of its bodies.
    pub(crate) fn wake_island(&mut self, island_id: IslandId) {
        let Some(island) = self.islands.get_mut(island_id) else {
            return;
        };
        if !island.asleep {
            return;
        }
        island.asleep = false;
        let bodies = island.bodies.clone();
        for body_id in bodies {
            if let Some(body) = self.bodies.get_mut(body_id) {
                body.asleep = false;
                body.sleep_time = 0.0;
            }
        }
    }
}
