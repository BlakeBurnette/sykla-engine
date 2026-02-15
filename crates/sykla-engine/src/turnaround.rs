use crate::physics::gradient_at;
use crate::terrain::{curvature_at, interpolate_point, offset_position, RoutePoint};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDirection {
    Forward,
    Reverse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteTopology {
    Loop,
    OutAndBack,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RiderPhase {
    OnRoute,
    UTurn {
        progress: f32,
        start_distance: f32,
        from_direction: RouteDirection,
        arc_center_x: f32,
        arc_center_z: f32,
        arc_radius: f32,
        start_angle: f32,
    },
}

pub struct ResolvedPosition {
    pub world_x: f32,
    pub world_z: f32,
    pub elevation: f32,
    pub forward_x: f32,
    pub forward_z: f32,
    pub lateral_offset: f32,
    pub gradient: f32,
    pub curvature: f32,
}

impl ResolvedPosition {
    pub fn offset_world_pos(&self) -> (f32, f32) {
        offset_position(
            self.world_x,
            self.world_z,
            self.forward_x,
            self.forward_z,
            self.lateral_offset,
        )
    }
}

pub struct RiderRouteState {
    pub direction: RouteDirection,
    pub phase: RiderPhase,
    pub topology: RouteTopology,
    pub lateral_offset: f32,
}

/// Lane center offset from road centerline.
const LANE_OFFSET: f32 = 2.5;

pub fn detect_topology(points: &[RoutePoint]) -> RouteTopology {
    if points.len() < 2 {
        return RouteTopology::OutAndBack;
    }
    let first = &points[0];
    let last = &points[points.len() - 1];
    let dx = last.pos_x - first.pos_x;
    let dz = last.pos_z - first.pos_z;
    let dist_sq = dx * dx + dz * dz;
    if dist_sq < 50.0 * 50.0 {
        RouteTopology::Loop
    } else {
        RouteTopology::OutAndBack
    }
}

impl RiderRouteState {
    pub fn new(points: &[RoutePoint]) -> Self {
        Self {
            direction: RouteDirection::Forward,
            phase: RiderPhase::OnRoute,
            topology: detect_topology(points),
            lateral_offset: LANE_OFFSET,
        }
    }

    /// Resolve the rider's current position on the route (no U-turn active).
    pub fn resolve_on_route(
        &self,
        distance: f32,
        points: &[RoutePoint],
    ) -> ResolvedPosition {
        let dist = distance as f64;
        let (px, pz, elev, raw_fx, raw_fz) = interpolate_point(points, dist);
        let raw_gradient = gradient_at(points, distance);
        let raw_curvature = curvature_at(points, dist);

        let (fx, fz, gradient, curvature) = match self.direction {
            RouteDirection::Forward => (raw_fx, raw_fz, raw_gradient, raw_curvature),
            RouteDirection::Reverse => (-raw_fx, -raw_fz, -raw_gradient, -raw_curvature),
        };

        ResolvedPosition {
            world_x: px,
            world_z: pz,
            elevation: elev,
            forward_x: fx,
            forward_z: fz,
            lateral_offset: self.lateral_offset,
            gradient,
            curvature,
        }
    }

    /// Begin a U-turn at the current distance.
    pub fn begin_uturn(&mut self, distance: f32, points: &[RoutePoint]) {
        if self.phase != RiderPhase::OnRoute {
            return;
        }

        let dist = distance as f64;
        let (px, pz, _elev, raw_fx, raw_fz) = interpolate_point(points, dist);

        // Forward direction (possibly flipped for reverse)
        let (fx, fz) = match self.direction {
            RouteDirection::Forward => (raw_fx, raw_fz),
            RouteDirection::Reverse => (-raw_fx, -raw_fz),
        };

        // Perpendicular: positive = right of forward
        let perp_x = -fz;
        let perp_z = fx;

        // Rider is at lateral_offset from centerline
        let rider_x = px + perp_x * self.lateral_offset;
        let rider_z = pz + perp_z * self.lateral_offset;

        // Arc center is at the road centerline (lateral_offset = 0)
        // Actually the arc center is between the two lanes — at the centerline
        let arc_center_x = px;
        let arc_center_z = pz;
        let arc_radius = LANE_OFFSET;

        // Start angle: angle from arc center to rider position
        let dx = rider_x - arc_center_x;
        let dz = rider_z - arc_center_z;
        let start_angle = dz.atan2(dx);

        self.phase = RiderPhase::UTurn {
            progress: 0.0,
            start_distance: distance,
            from_direction: self.direction,
            arc_center_x,
            arc_center_z,
            arc_radius,
            start_angle,
        };
    }

    /// Update the rider state each frame. Returns the resolved position.
    pub fn update(
        &mut self,
        physics_distance: f32,
        physics_speed: f32,
        points: &[RoutePoint],
        _route_length: f32,
        dt: f32,
    ) -> ResolvedPosition {
        // Smooth lateral offset toward target lane
        let target_lateral = match self.direction {
            RouteDirection::Forward => LANE_OFFSET,
            RouteDirection::Reverse => -LANE_OFFSET,
        };
        if self.phase == RiderPhase::OnRoute {
            let rate = 3.0; // smoothing rate
            let alpha = 1.0 - (-rate * dt).exp();
            self.lateral_offset += (target_lateral - self.lateral_offset) * alpha;
        }

        match self.phase {
            RiderPhase::OnRoute => self.resolve_on_route(physics_distance, points),
            RiderPhase::UTurn {
                ref mut progress,
                start_distance,
                from_direction,
                arc_center_x,
                arc_center_z,
                arc_radius,
                start_angle,
            } => {
                // Advance arc progress
                let speed = physics_speed.max(2.0);
                let arc_length = std::f32::consts::PI * arc_radius;
                let dp = (speed * dt) / arc_length;
                *progress = (*progress + dp).min(1.0);

                let p = *progress;

                // Sweep direction: Forward→Reverse sweeps counterclockwise (left U-turn)
                // Reverse→Forward sweeps clockwise
                let sweep = match from_direction {
                    RouteDirection::Forward => std::f32::consts::PI,  // CCW
                    RouteDirection::Reverse => -std::f32::consts::PI, // CW
                };

                let angle = start_angle + sweep * p;
                let rider_x = arc_center_x + arc_radius * angle.cos();
                let rider_z = arc_center_z + arc_radius * angle.sin();

                // Tangent to arc (perpendicular to radius, in sweep direction)
                let tangent_x = -angle.sin() * sweep.signum();
                let tangent_z = angle.cos() * sweep.signum();
                let len = (tangent_x * tangent_x + tangent_z * tangent_z).sqrt().max(0.0001);

                // Get elevation from the route at start_distance
                let (_, _, elev, _, _) = interpolate_point(points, start_distance as f64);

                let resolved = ResolvedPosition {
                    world_x: rider_x,
                    world_z: rider_z,
                    elevation: elev,
                    forward_x: tangent_x / len,
                    forward_z: tangent_z / len,
                    lateral_offset: 0.0, // already at world position during U-turn
                    gradient: 0.0,
                    curvature: 1.0 / arc_radius * sweep.signum(),
                };

                // Complete U-turn
                if p >= 1.0 {
                    let new_direction = match from_direction {
                        RouteDirection::Forward => RouteDirection::Reverse,
                        RouteDirection::Reverse => RouteDirection::Forward,
                    };
                    self.direction = new_direction;
                    self.phase = RiderPhase::OnRoute;
                    self.lateral_offset = match new_direction {
                        RouteDirection::Forward => LANE_OFFSET,
                        RouteDirection::Reverse => -LANE_OFFSET,
                    };
                }

                resolved
            }
        }
    }
}
