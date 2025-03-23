use std::collections::HashMap;
use std::marker::PhantomData;

use crate::core::{ActorState, ActorStateType};

// Light bulb states
#[derive(Clone, Debug)]
pub struct On;
#[derive(Clone, Debug)]
pub struct Off;

type DispatchMap<S> = HashMap<&'static str, fn(&LightBulb<S>, f32) -> Box<ActorStateType>>;

#[derive(Clone, Debug)]
pub struct LightBulb<State> {
    dispatch_map: DispatchMap<State>,
    threshold: f32,
    _state: PhantomData<State>,
}

// TODO: generate with a macro
impl<T> LightBulb<T> {
    /// Create a new LightBulb default state with the given threshold
    pub fn create(threshold: f32) -> Box<ActorStateType> {
        Box::new(LightBulb {
            dispatch_map: LightBulb::<Off>::create_dispatch_map(),
            threshold,
            _state: PhantomData::<Off>,
        })
    }

    pub fn slots() -> Vec<&'static str> {
        vec!["CurrentPowerDraw"]
    }
}

// TODO: generate with a macro
impl ActorState for LightBulb<On> {
    fn input_change(&self, slot: &str, value: f32) -> Box<ActorStateType> {
        match self.dispatch_map.get(slot) {
            Some(func) => func(self, value),
            None => Box::new((*self).clone()),
        }
    }

    fn state(&self) -> String {
        "On".to_string()
    }

    fn type_name(&self) -> String {
        "LightBulb".to_string()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// TODO: generate with a macro
impl LightBulb<On> {
    fn create_dispatch_map() -> DispatchMap<On> {
        let mut dispatch_map: DispatchMap<On> = HashMap::new();
        dispatch_map.insert("CurrentPowerDraw", LightBulb::<On>::power_change);
        dispatch_map
    }
}

impl LightBulb<On> {
    fn power_change(&self, pwr: f32) -> Box<ActorStateType> {
        if pwr < self.threshold {
            Box::new(LightBulb {
                dispatch_map: LightBulb::<Off>::create_dispatch_map(),
                threshold: self.threshold,
                _state: PhantomData::<Off>,
            })
        } else {
            Box::new(LightBulb {
                dispatch_map: LightBulb::<On>::create_dispatch_map(),
                threshold: self.threshold,
                _state: PhantomData::<On>,
            })
        }
    }
}

// TODO: generate with a macro
impl ActorState for LightBulb<Off> {
    fn input_change(&self, slot: &str, value: f32) -> Box<ActorStateType> {
        match self.dispatch_map.get(slot) {
            Some(func) => func(self, value),
            None => Box::new((*self).clone()),
        }
    }

    fn state(&self) -> String {
        "Off".to_string()
    }

    fn type_name(&self) -> String {
        "LightBulb".to_string()
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

// TODO: generate with a macro
impl LightBulb<Off> {
    fn create_dispatch_map() -> DispatchMap<Off> {
        let mut dispatch_map: DispatchMap<Off> = HashMap::new();
        dispatch_map.insert("CurrentPowerDraw", LightBulb::<Off>::power_change);
        dispatch_map
    }
}

impl LightBulb<Off> {
    fn power_change(&self, pwr: f32) -> Box<ActorStateType> {
        if pwr >= self.threshold {
            Box::new(LightBulb {
                dispatch_map: LightBulb::<On>::create_dispatch_map(),
                threshold: self.threshold,
                _state: PhantomData::<On>,
            })
        } else {
            Box::new(LightBulb {
                dispatch_map: LightBulb::<Off>::create_dispatch_map(),
                threshold: self.threshold,
                _state: PhantomData::<Off>,
            })
        }
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
