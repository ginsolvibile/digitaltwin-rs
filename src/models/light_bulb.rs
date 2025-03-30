use std::collections::HashMap;
use std::marker::PhantomData;

use crate::core::{ActorState, ActorStateType, StateBehavior};
use crate::{define_state_maps, impl_actor_state};

// Light bulb states
#[derive(Clone, Debug)]
pub struct On;
#[derive(Clone, Debug)]
pub struct Off;

/// The default state of the light bulb
pub type LightBulbDefault = LightBulb<Off>;

// Define dispatch and command map types specific to LightBulb
type DispatchMap<S> = HashMap<&'static str, fn(&LightBulb<S>, f32) -> Box<ActorStateType>>;
type CommandMap<S> = HashMap<&'static str, fn(&LightBulb<S>, serde_json::Value) -> Box<ActorStateType>>;

#[derive(Clone, Debug)]
pub struct LightBulb<State> {
    dispatch_map: DispatchMap<State>,
    command_map: CommandMap<State>,
    threshold: f32,
    _state: PhantomData<State>,
}

impl<State> LightBulb<State>
where
    State: Send + Sync + 'static,
    LightBulb<State>: ActorState,
{
    pub fn create(threshold: f32) -> Box<ActorStateType> {
        Box::new(LightBulb {
            dispatch_map: Off::create_dispatch_map(),
            command_map: Off::create_command_map(),
            threshold,
            _state: PhantomData::<_>,
        })
    }

    fn transition<T>(&self) -> Box<ActorStateType>
    where
        LightBulb<T>: ActorState,
        T: StateBehavior<Actor = LightBulb<T>> + Send + Sync + 'static,
    {
        Box::new(LightBulb {
            dispatch_map: T::create_dispatch_map(),
            command_map: T::create_command_map(),
            threshold: self.threshold,
            _state: PhantomData::<_>,
        })
    }

    pub fn slots() -> Vec<&'static str> {
        vec!["CurrentPowerDraw"]
    }
}

// Apply the macro for ActorState implementation
impl_actor_state!(LightBulb, On, "On");
impl_actor_state!(LightBulb, Off, "Off");

// Apply the macro for dispatch and command maps
define_state_maps!(
    LightBulb,
    On,
    [("CurrentPowerDraw", LightBulb::<On>::power_change)],
    [("SwitchOff", LightBulb::<On>::switch_off)]
);

define_state_maps!(
    LightBulb,
    Off,
    [("CurrentPowerDraw", LightBulb::<Off>::power_change)],
    [("SwitchOn", LightBulb::<Off>::switch_on)]
);

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
