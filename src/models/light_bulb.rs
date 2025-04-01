use std::collections::HashMap;
use std::marker::PhantomData;

use crate::core::{ActorFactory, ActorState, ActorStateType, CommandMap, DispatchMap, StateBehavior};
use crate::{declare_slots, define_state_maps, impl_actor_state};

// Light bulb states
#[derive(Clone, Debug)]
pub struct On;
#[derive(Clone, Debug)]
pub struct Off;

/// Factory for creating LightBulb actors
pub struct LightBulbFactory;
impl ActorFactory for LightBulbFactory {
    fn create_default() -> (Box<ActorStateType>, Vec<&'static str>) {
        (LightBulb::<Off>::create(0.5), LightBulb::<Off>::slots())
    }

    fn create_with_params(params: serde_json::Value) -> (Box<ActorStateType>, Vec<&'static str>) {
        let threshold = params
            .get("threshold")
            .and_then(|v| v.as_f64())
            .map(|v| v as f32)
            .unwrap_or(0.5);

        (LightBulb::<Off>::create(threshold), LightBulb::<Off>::slots())
    }
}

#[derive(Clone, Debug)]
pub struct LightBulb<State> {
    threshold: f32,
    dispatch_map: DispatchMap<LightBulb<State>>,
    command_map: CommandMap<LightBulb<State>>,
    _state: PhantomData<State>,
}

impl<State> LightBulb<State>
where
    State: Send + Sync + 'static,
    LightBulb<State>: ActorState,
{
    /// Create the default instance of a LightBulb actor
    pub fn create(threshold: f32) -> Box<ActorStateType> {
        Box::new(LightBulb {
            threshold,
            dispatch_map: Off::create_dispatch_map(),
            command_map: Off::create_command_map(),
            _state: PhantomData::<_>,
        })
    }

    /// The `transition` method returns a new instance of the actor with the specified state,
    /// inheriting the actor's properties.
    fn transition<T>(&self) -> Box<ActorStateType>
    where
        LightBulb<T>: ActorState,
        T: StateBehavior<Actor = LightBulb<T>> + Send + Sync + 'static,
    {
        Box::new(LightBulb {
            threshold: self.threshold,
            dispatch_map: T::create_dispatch_map(),
            command_map: T::create_command_map(),
            _state: PhantomData::<_>,
        })
    }
}

// Apply the macro for ActorState implementation
impl_actor_state!(LightBulb);

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
