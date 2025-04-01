use std::collections::HashMap;
use std::marker::PhantomData;

use crate::core::{ActorFactory, ActorState, ActorStateType, CommandMap, DispatchMap, StateBehavior};
use crate::{declare_slots, define_actor, define_actor_factory, define_state_maps, impl_actor_state};

// Light bulb states
#[derive(Clone, Debug)]
pub struct On;
#[derive(Clone, Debug)]
pub struct Off;

// Define the LightBulb actor with its default state
define_actor!(
    LightBulb {
        threshold: f32 = 0.5,
    }, Off
);

// Factory for creating LightBulb actors
define_actor_factory!(
    LightBulb, LightBulbFactory, 
    Off, 
    (threshold: f32 = 0.5)
);

// Declare inputs variables for the LightBulb actor
declare_slots!(LightBulb, ["CurrentPowerDraw"]);

// define handlers for each input slot in the On state
define_state_maps!(
    LightBulb,
    On,
    [("CurrentPowerDraw", power_change)],
    [("SwitchOff", switch_off)]
);

// define handlers for each input slot in the Off state
define_state_maps!(
    LightBulb,
    Off,
    [("CurrentPowerDraw", power_change)],
    [("SwitchOn", switch_on)]
);

// Input and command handlers for the On state
impl LightBulb<On> {
    fn power_change(&self, pwr: f32) -> Box<ActorStateType> {
        if pwr < self.threshold {
            self.transition::<Off>()
        } else {
            self.transition::<On>()
        }
    }

    fn switch_off(&self, _arg: serde_json::Value) -> Box<ActorStateType> {
        self.transition::<Off>()
    }
}

// Input and command handlers for the Off state
impl LightBulb<Off> {
    fn power_change(&self, pwr: f32) -> Box<ActorStateType> {
        if pwr >= self.threshold {
            self.transition::<On>()
        } else {
            self.transition::<Off>()
        }
    }

    fn switch_on(&self, _arg: serde_json::Value) -> Box<ActorStateType> {
        self.transition::<On>()
    }
}

#[cfg(test)]
mod tests {
    use crate::models::light_bulb::{LightBulb, Off, On};

    #[test]
    fn test_power_change() {
        let actor = LightBulb::<Off>::create(0.5);

        let actor = actor.input_change("power", 0.3);
        println!("After power change of 0.3: {:?}", actor);
        assert!(actor.as_any().downcast_ref::<LightBulb<Off>>().is_some());

        let actor = actor.input_change("power", 0.7);
        println!("After power change of 0.7: {:?}", actor);
        assert!(actor.as_any().downcast_ref::<LightBulb<On>>().is_some());

        let actor = actor.input_change("power", 0.3);
        println!("After power change of 0.3: {:?}", actor);
        assert!(actor.as_any().downcast_ref::<LightBulb<Off>>().is_some());
    }
}
