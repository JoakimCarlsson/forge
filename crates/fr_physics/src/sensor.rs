//! Sensors: shapes that detect overlaps without colliding.

use crate::constants::OVERLAP_SLOP;
use crate::distance::{DistanceInput, SimplexCache, shape_distance};
use crate::shape::ShapeId;
use crate::world::{SensorEvent, World};

impl World {
    /// Recomputes the overlaps of every sensor and records begin and end events.
    pub(crate) fn update_sensors(&mut self) {
        let sensors: Vec<ShapeId> = self
            .shapes
            .iter()
            .filter(|(_, shape)| shape.is_sensor)
            .map(|(id, _)| id)
            .collect();
        for sensor_id in sensors {
            let overlaps = self.compute_sensor_overlaps(sensor_id);
            let Some(sensor) = self.shapes.get_mut(sensor_id) else {
                continue;
            };
            let previous = std::mem::take(&mut sensor.sensor_overlaps);
            for &visitor in &overlaps {
                if previous.binary_search(&visitor).is_err() {
                    self.sensor_begin_events.push(SensorEvent {
                        sensor: sensor_id,
                        visitor,
                    });
                }
            }
            for &visitor in &previous {
                if overlaps.binary_search(&visitor).is_err() {
                    self.sensor_end_events.push(SensorEvent {
                        sensor: sensor_id,
                        visitor,
                    });
                }
            }
            if let Some(sensor) = self.shapes.get_mut(sensor_id) {
                sensor.sensor_overlaps = overlaps;
            }
        }
    }

    /// The shapes a sensor overlaps, sorted by handle.
    fn compute_sensor_overlaps(&self, sensor_id: ShapeId) -> Vec<ShapeId> {
        let Some(sensor) = self.shapes.get(sensor_id) else {
            return Vec::new();
        };
        let Some(sensor_body) = self.bodies.get(sensor.body) else {
            return Vec::new();
        };
        let sensor_pose = sensor_body.pose;
        let aabb = sensor.geometry.compute_aabb(&sensor_pose);
        let mut slots: Vec<u32> = Vec::new();
        self.broad_phase.query_all(&aabb, |slot| {
            slots.push(slot);
            true
        });
        let mut overlaps = Vec::new();
        for slot in slots {
            let Some(other_id) = self.shapes.handle_at(slot as usize) else {
                continue;
            };
            let Some(other) = self.shapes.get(other_id) else {
                continue;
            };
            if other.is_sensor
                || other.body == sensor.body
                || !other.enable_sensor_events
                || !sensor.filter.should_collide(&other.filter)
            {
                continue;
            }
            let Some(other_body) = self.bodies.get(other.body) else {
                continue;
            };
            let input = DistanceInput {
                proxy_a: sensor.geometry.proxy(),
                proxy_b: other.geometry.proxy(),
                transform: sensor_pose.inv_mul(&other_body.pose),
                use_radii: true,
            };
            let mut cache = SimplexCache::default();
            if shape_distance(&input, &mut cache).distance < OVERLAP_SLOP {
                overlaps.push(other_id);
            }
        }
        overlaps.sort();
        overlaps
    }
}
