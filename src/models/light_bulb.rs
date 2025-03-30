use std::collections::HashMap;
use std::marker::PhantomData;

use crate::core::{ActorState, ActorStateType};

// Light bulb states
#[derive(Clone, Debug)]
pub struct On;
#[derive(Clone, Debug)]
pub struct Off;

/// The default state of the light bulb
pub type LightBulbDefault = LightBulb<Off>;

type DispatchMap<S> = HashMap<&'static str, fn(&LightBulb<S>, f32) -> Box<ActorStateType>>;
type CommandMap<S> = HashMap<&'static str, fn(&LightBulb<S>, serde_json::Value) -> Box<ActorStateType>>;

#[derive(Clone, Debug)]
pub struct LightBulb<State> {
    dispatch_map: DispatchMap<State>,
    command_map: CommandMap<State>,
    threshold: f32,
    _state: PhantomData<State>,
}

// Define a macro for implementing ActorState trait
macro_rules! impl_actor_state {
    ($actor:ident, $state:ty, $state_name:expr) => {
        impl ActorState for $actor<$state> {
            fn input_change(&self, slot: &str, value: f32) -> Box<ActorStateType> {
                match self.dispatch_map.get(slot) {
                    Some(func) => func(self, value),
                    // TODO: notify error
                    None => Box::new((*self).clone()),
                }
            }

            fn execute(&self, command: &str, arg: serde_json::Value) -> Box<ActorStateType> {
                match self.command_map.get(command) {
                    Some(func) => func(self, arg),
                    // TODO: notify error
                    None => Box::new((*self).clone()),
                }
            }

            fn state(&self) -> String {
                $state_name.to_string()
            }

            fn type_name(&self) -> String {
                stringify!($actor).to_string()
            }

            fn as_any(&self) -> &dyn std::any::Any {
                self
            }
        }
    };
}

// Define a macro for dispatch and command maps
macro_rules! define_state_maps {
    ($actor:ident, $state:ty, $dispatch_entries:expr, $command_entries:expr) => {
        impl $actor<$state> {
            fn create_dispatch_map() -> DispatchMap<$state> {
                let mut dispatch_map: DispatchMap<$state> = HashMap::new();
                for (slot, handler) in $dispatch_entries {
                    dispatch_map.insert(slot, handler);
                }
                dispatch_map
            }

            fn create_command_map() -> CommandMap<$state> {
                let mut command_map: CommandMap<$state> = HashMap::new();
                for (cmd, handler) in $command_entries {
                    command_map.insert(cmd, handler);
                }
                command_map
            }
        }
    };
}

pub trait StateBehavior {
    fn create_dispatch_map() -> DispatchMap<Self>
    where
        Self: Sized;
    fn create_command_map() -> CommandMap<Self>
    where
        Self: Sized;
}

impl StateBehavior for On {
    fn create_dispatch_map() -> DispatchMap<Self> {
        let mut dispatch_map: DispatchMap<Self> = HashMap::new();
        dispatch_map.insert("CurrentPowerDraw", LightBulb::<On>::power_change);
        dispatch_map
    }

    fn create_command_map() -> CommandMap<Self> {
        let mut command_map: CommandMap<Self> = HashMap::new();
        command_map.insert("SwitchOff", LightBulb::<On>::switch_off);
        command_map
    }
}

impl StateBehavior for Off {
    fn create_dispatch_map() -> DispatchMap<Self> {
        let mut dispatch_map: DispatchMap<Self> = HashMap::new();
        dispatch_map.insert("CurrentPowerDraw", LightBulb::<Off>::power_change);
        dispatch_map
    }

    fn create_command_map() -> CommandMap<Self> {
        let mut command_map: CommandMap<Self> = HashMap::new();
        command_map.insert("SwitchOn", LightBulb::<Off>::switch_on);
        command_map
    }
}

impl<T> LightBulb<T>
where
    T: StateBehavior + Send + Sync + 'static,
    LightBulb<T>: ActorState,
{
    pub fn create(threshold: f32) -> Box<ActorStateType> {
        Box::new(LightBulb {
            dispatch_map: T::create_dispatch_map(),
            command_map: T::create_command_map(),
            threshold,
            _state: PhantomData::<T>,
        })
    }

    pub fn slots() -> Vec<&'static str> {
        vec!["CurrentPowerDraw"]
    }
}

// Apply the macro for ActorState implementation - updated with actor type parameter
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
    fn transition<T: Send + Sync>(&self) -> Box<ActorStateType>
    where
        LightBulb<T>: ActorState,
    {
        Box::new(LightBulb {
            dispatch_map: LightBulb::<On>::create_dispatch_map(),
            command_map: LightBulb::<On>::create_command_map(),
            threshold: self.threshold,
            _state: PhantomData::<_>,
        })
    }
}

impl LightBulb<Off> {
    fn transition<T: Send + Sync>(&self) -> Box<ActorStateType>
    where
        LightBulb<T>: ActorState,
    {
        Box::new(LightBulb {
            dispatch_map: LightBulb::<Off>::create_dispatch_map(),
            command_map: LightBulb::<Off>::create_command_map(),
            threshold: self.threshold,
            _state: PhantomData::<_>,
        })
    }
}

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
