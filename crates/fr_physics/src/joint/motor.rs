//! The motor every joint can be driven by: a torque limited spring towards a target pose plus
//! the joint friction that resists motion.

use fr_math::{Quat, Vec3};

use crate::joint::JointKind;
use crate::math::twist_angle;

/// How hard a joint drives itself towards its target pose.
///
/// The spring pulls the relative rotation of the two bodies towards the target. With zero
/// `strength` the joint is limp and only its limits and friction act.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JointMotor {
    /// The stiffness of the spring in hertz; zero is limp.
    pub strength: f32,
    /// The damping ratio of the spring.
    pub damping: f32,
    /// The largest torque the spring may apply; a force for a distance joint.
    pub max_torque: f32,
}

impl JointMotor {
    /// A motor that does not drive the joint.
    pub const LIMP: Self = Self {
        strength: 0.0,
        damping: 0.7,
        max_torque: f32::MAX,
    };

    /// A motor of `strength` hertz, damping ratio `damping` and torque limit `max_torque`.
    pub const fn new(strength: f32, damping: f32, max_torque: f32) -> Self {
        Self {
            strength,
            damping,
            max_torque,
        }
    }
}

impl Default for JointMotor {
    /// A limp motor with the damping ratio of Box3D's ragdoll sample and no torque limit.
    fn default() -> Self {
        Self::LIMP
    }
}

impl JointKind {
    /// Sets the spring of the joint from `motor`.
    ///
    /// A spherical or revolute joint springs towards its target rotation and a distance joint
    /// towards its length; both stop springing at zero strength. A mouse joint takes the strength as
    /// its hertz, the damping as its damping ratio and the torque limit as its largest force. A weld holds its frames
    /// together, so there the strength is the softness of the angular constraint and zero is
    /// rigid. A filter joint has no motor.
    pub fn set_motor(&mut self, motor: &JointMotor) {
        match self {
            Self::Spherical(joint) => {
                joint.enable_spring = motor.strength > 0.0;
                if !joint.enable_spring {
                    joint.spring_impulse = Vec3::ZERO;
                }
                joint.hertz = motor.strength;
                joint.damping_ratio = motor.damping;
                joint.max_spring_torque = motor.max_torque;
            }
            Self::Revolute(joint) => {
                joint.enable_spring = motor.strength > 0.0;
                if !joint.enable_spring {
                    joint.spring_impulse = 0.0;
                }
                joint.hertz = motor.strength;
                joint.damping_ratio = motor.damping;
                joint.max_spring_torque = motor.max_torque;
            }
            Self::Weld(joint) => {
                joint.angular_hertz = motor.strength;
                joint.angular_damping_ratio = motor.damping;
            }
            Self::Distance(joint) => {
                joint.enable_spring = true;
                joint.hertz = motor.strength;
                joint.damping_ratio = motor.damping;
                joint.lower_spring_force = -motor.max_torque;
                joint.upper_spring_force = motor.max_torque;
            }
            Self::Mouse(joint) => {
                joint.hertz = motor.strength;
                joint.damping_ratio = motor.damping;
                joint.max_force = motor.max_torque;
            }
            Self::Filter => {}
        }
    }

    /// Sets the torque that resists motion of the joint at zero speed, Box3D's joint friction.
    ///
    /// A distance joint resists with a force; a weld, a mouse and a filter joint have none.
    pub fn set_friction(&mut self, torque: f32) {
        match self {
            Self::Spherical(joint) => {
                joint.enable_motor = true;
                joint.max_motor_torque = torque;
            }
            Self::Revolute(joint) => {
                joint.enable_motor = true;
                joint.max_motor_torque = torque;
            }
            Self::Distance(joint) => {
                joint.enable_motor = true;
                joint.max_motor_force = torque;
            }
            Self::Weld(_) | Self::Mouse(_) | Self::Filter => {}
        }
    }

    /// Sets the pose the spring drives towards, as the rotation of the joint frame on body B in
    /// the joint frame on body A; the identity is the pose the joint was built in.
    ///
    /// A revolute joint takes the rotation about its hinge axis; the other kinds ignore it.
    pub fn set_target_rotation(&mut self, relative: Quat) {
        match self {
            Self::Spherical(joint) => joint.target_rotation = relative,
            Self::Revolute(joint) => joint.target_angle = twist_angle(relative),
            Self::Weld(_) | Self::Distance(_) | Self::Mouse(_) | Self::Filter => {}
        }
    }
}
