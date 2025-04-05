use digitaltwin_core::ActorStateType;
use digitaltwin_macros::*;

// Charging Station states

/// No vehicle connected
#[derive(Clone, Debug)]
pub struct Idle;

/// Vehicle connected, not charging
#[derive(Clone, Debug)]
pub struct Connected;

/// Vehicle connected, charging
#[derive(Clone, Debug)]
pub struct Charging;

/// Device is in fault state
#[derive(Clone, Debug)]
pub struct Fault;

#[actor(default_state = "Idle", slots("CurrentPowerDraw", "InputCurrent"))]
pub struct ChargingStation {
    /// minimum current draw when in charging mode [A]
    #[actor_attr(default = "1.0")]
    min_current: f32,
    /// max current draw when in charging mode [A]
    #[actor_attr(default = "16.0")]
    max_current: f32,
    /// max power draw when in sleep mode [W]
    #[actor_attr(default = "5.0")]
    max_sleep_power: f32,
}

#[actor_state(ChargingStation, Idle)]
#[dispatch_map("CurrentPowerDraw" = power_change)]
#[command_map("VehicleDetected" = connect_vehicle)]
impl ChargingStation<Idle> {
    // When in idle state, the power draw should be nearly 0.
    // Otherwise, we assume a fault is present
    fn power_change(&self, pwr: f32) -> Box<ActorStateType> {
        if pwr > self.max_sleep_power {
            // TODO: raise invalid power absorbtion event
            self.transition::<Fault>()
        } else {
            self.transition::<Idle>()
        }
    }

    // A vehicle is detected, go to connected state
    fn connect_vehicle(&self, _arg: serde_json::Value) -> Box<ActorStateType> {
        self.transition::<Connected>()
    }
}

#[actor_state(ChargingStation, Connected)]
#[dispatch_map("InputCurrent" = current_change)]
#[command_map("VehicleDisconnected" = disconnect_vehicle)]
impl ChargingStation<Connected> {
    // When in connected state, if detect a power draw
    // we assume the vehicle is charging
    fn current_change(&self, current: f32) -> Box<ActorStateType> {
        if current > self.min_current {
            self.transition::<Charging>()
        } else {
            self.transition::<Connected>()
        }
    }

    // The vehicle is disconnected, go to idle state
    fn disconnect_vehicle(&self, _arg: serde_json::Value) -> Box<ActorStateType> {
        self.transition::<Idle>()
    }
}

#[actor_state(ChargingStation, Charging)]
#[dispatch_map("CurrentPowerDraw" = power_change)]
#[dispatch_map("InputCurrent" = current_change)]
#[command_map("SetChargingCurrent" = set_charging_current)]
impl ChargingStation<Charging> {
    // If power goes below the minimum threshold
    // we assume charging is complete (or the user has stopped charging)
    fn power_change(&self, pwr: f32) -> Box<ActorStateType> {
        if pwr < self.max_sleep_power {
            // TODO: raise "charging complete" event
            self.transition::<Connected>()
        } else {
            self.transition::<Charging>()
        }
    }

    // If an overcurrent is detected, we assume a fault is present
    fn current_change(&self, current: f32) -> Box<ActorStateType> {
        if current > self.max_current {
            self.transition::<Fault>()
        } else {
            self.transition::<Charging>()
        }
    }

    // Set the charging current to a new value
    fn set_charging_current(&self, arg: serde_json::Value) -> Box<ActorStateType> {
        log::info!("Set charging current to {}", arg);
        // TODO: send a "set current" command to the device
        self.transition::<Charging>()
    }
}

#[actor_state(ChargingStation, Fault)]
#[command_map("Reset" = reset)]
impl ChargingStation<Fault> {
    // Reset the fault state and go to idle state
    fn reset(&self, _arg: serde_json::Value) -> Box<ActorStateType> {
        self.transition::<Idle>()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use digitaltwin_core::ActorFactory;

    #[test]
    fn test_idle_state_power_change_high() {
        let (actor, _) = ChargingStationFactory::create_default();
        let actor = actor.input_change("CurrentPowerDraw", 10.0);
        // Expect transition to Fault
        assert!(actor.as_any().downcast_ref::<ChargingStation<Fault>>().is_some());
    }

    #[test]
    fn test_idle_state_vehicle_detected() {
        let (actor, _) = ChargingStationFactory::create_default();
        let actor = actor.execute("VehicleDetected", serde_json::json!({}));
        // Expect transition to Connected
        assert!(actor
            .as_any()
            .downcast_ref::<ChargingStation<Connected>>()
            .is_some());
    }

    #[test]
    fn test_charging_state_complete() {
        let (actor, _) = ChargingStationFactory::create_default();
        let actor = actor
            // Connect vehicle
            .execute("VehicleDetected", serde_json::json!({}))
            // Go to charging state
            .input_change("InputCurrent", 10.0)
            // Emulate power draw going to 1 W
            .input_change("CurrentPowerDraw", 1.0);
        // Expect final state to be Connected
        assert!(actor
            .as_any()
            .downcast_ref::<ChargingStation<Connected>>()
            .is_some());
    }

    #[test]
    fn test_connected_state_current_change_high() {
        let (actor, _) = ChargingStationFactory::create_default();
        let actor = actor
            // Connect vehicle
            .execute("VehicleDetected", serde_json::json!({}))
            // Go to charging state
            .input_change("InputCurrent", 10.0);
        // Expect transition to Charging
        assert!(actor
            .as_any()
            .downcast_ref::<ChargingStation<Charging>>()
            .is_some());
    }

    #[test]
    fn test_charging_state_overcurrent() {
        let (actor, _) = ChargingStationFactory::create_default();
        let actor = actor
            // Connect vehicle
            .execute("VehicleDetected", serde_json::json!({}))
            // Go to charging state
            .input_change("InputCurrent", 10.0)
            // Emulate overcurrent
            .input_change("InputCurrent", 20.0);
        // Expect transition to Fault
        assert!(actor.as_any().downcast_ref::<ChargingStation<Fault>>().is_some());
    }

    #[test]
    fn test_fault_state_reset() {
        let (actor, _) = ChargingStationFactory::create_default();
        let actor = actor
            // Connect vehicle
            .execute("VehicleDetected", serde_json::json!({}))
            // Go to charging state
            .input_change("InputCurrent", 10.0)
            // Emulate overcurrent
            .input_change("InputCurrent", 20.0)
            // Reset fault
            .execute("Reset", serde_json::json!({}));
        // Expect transition back to Idle
        assert!(actor.as_any().downcast_ref::<ChargingStation<Idle>>().is_some());
    }
}
