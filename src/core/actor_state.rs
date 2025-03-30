use std::collections::HashMap;

pub type ActorStateType = dyn ActorState + Send + Sync + 'static;

pub trait ActorState {
    /// Handle the change of an input slot
    fn input_change(&self, slot: &str, value: f32) -> Box<ActorStateType>;
    /// Execute a command
    fn execute(&self, command: &str, input: serde_json::Value) -> Box<ActorStateType>;

    // Helper functions
    fn as_any(&self) -> &dyn std::any::Any;
    fn type_name(&self) -> String;
    fn state(&self) -> String;
}

impl std::fmt::Debug for Box<ActorStateType> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.type_name(), self.state())
    }
}

// Define a trait for state behaviors - must be implemented by state types
pub trait StateBehavior {
    // The actor type that uses this state
    type Actor;

    // Create dispatch and command maps
    fn create_dispatch_map() -> HashMap<&'static str, fn(&Self::Actor, f32) -> Box<ActorStateType>>;
    fn create_command_map(
    ) -> HashMap<&'static str, fn(&Self::Actor, serde_json::Value) -> Box<ActorStateType>>;
}

// Define a macro for implementing ActorState trait
#[macro_export]
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

#[macro_export]
macro_rules! define_state_maps {
    ($actor:ident, $state:ty, [$(($d_slot:expr, $d_handler:expr)),*], [$(($c_slot:expr, $c_handler:expr)),*]) => {
        impl StateBehavior for $state {
            type Actor = $actor<$state>;
            
            fn create_dispatch_map() -> HashMap<&'static str, fn(&Self::Actor, f32) -> Box<ActorStateType>> {
                let mut dispatch_map = HashMap::new();
                $(
                    dispatch_map.insert($d_slot, $d_handler as fn(&Self::Actor, f32) -> Box<ActorStateType>);
                )*
                dispatch_map
            }

            fn create_command_map() -> HashMap<&'static str, fn(&Self::Actor, serde_json::Value) -> Box<ActorStateType>> {
                let mut command_map = HashMap::new();
                $(
                    command_map.insert($c_slot, $c_handler as fn(&Self::Actor, serde_json::Value) -> Box<ActorStateType>);
                )*
                command_map
            }
        }
    };
}
