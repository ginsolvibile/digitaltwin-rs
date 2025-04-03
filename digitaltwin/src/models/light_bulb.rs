use digitaltwin_core::ActorStateType;
use digitaltwin_macros::*;

// Light bulb states
#[derive(Clone, Debug)]
pub struct On;
#[derive(Clone, Debug)]
pub struct Off;

/// The LightBulb actor
#[actor(default_state = "Off", slots("CurrentPowerDraw"))]
pub struct LightBulb {
    #[actor_attr(default = "0.5")]
    threshold: f32,
}

// Input and command handlers for the On state
#[actor_state(LightBulb, On)]
#[dispatch_map("CurrentPowerDraw" = power_change)]
#[command_map("SwitchOff" = switch_off)]
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
#[actor_state(LightBulb, Off)]
#[dispatch_map("CurrentPowerDraw" = power_change)]
#[command_map("SwitchOn" = switch_on)]
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
