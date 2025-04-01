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

/// Factory trait for creating actors. Each Actor type must implement this trait
/// to provide a default instance and a way to create instances with parameters.
pub trait ActorFactory {
    fn create_default() -> (Box<ActorStateType>, Vec<&'static str>);
    fn create_with_params(params: serde_json::Value) -> (Box<ActorStateType>, Vec<&'static str>);
}

/// State behavior trait for providing the input and command handler dispatch maps.
pub trait StateBehavior {
    /// The actor type that uses this state
    type Actor;

    /// Create the update dispatch map
    fn create_dispatch_map() -> DispatchMap<Self::Actor>;
    /// Create the command dispatch map
    fn create_command_map() -> CommandMap<Self::Actor>;

    fn state_name() -> String;
}

/// The dispatch map associates input slots (strings) with their handlers
pub type DispatchMap<A> = HashMap<&'static str, fn(&A, f32) -> Box<ActorStateType>>;
/// The command map associates commands (strings) with their handlers
pub type CommandMap<A> = HashMap<&'static str, fn(&A, serde_json::Value) -> Box<ActorStateType>>;

/// Implement the ActorState trait boilerplate for a given actor.
#[macro_export]
macro_rules! impl_actor_state {
    ($actor:ident) => {
        impl<S> ActorState for $actor<S>
        where
            S: StateBehavior + Clone + Send + Sync + 'static,
        {
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
                S::state_name()
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

/// Define input and command maps given the actor and state types.
/// This macro generates the necessary code to create the input and command maps for the specified actor and state.
/// Syntax:
/// ```
///     define_state_maps!(
///         ActorType,
///         StateType,
///         [
///             ("InputVarName", input_handler),
///             ...
///         ],
///         [
///             ("CommandName", command_handler),
///             ...
///         ]);
/// ```
#[macro_export]
macro_rules! define_state_maps {
    ($actor:ident, $state:ty, [$(($d_slot:expr, $d_handler:ident)),*], [$(($c_slot:expr, $c_handler:ident)),*]) => {
        impl StateBehavior for $state {
            type Actor = $actor<$state>;

            fn create_dispatch_map() -> DispatchMap<Self::Actor> {
                let mut dispatch_map = HashMap::new();
                $(
                    dispatch_map.insert($d_slot, $actor::<$state>::$d_handler as fn(&Self::Actor, f32) -> Box<ActorStateType>);
                )*
                dispatch_map
            }

            fn create_command_map() -> CommandMap<Self::Actor> {
                let mut command_map = HashMap::new();
                $(
                    command_map.insert($c_slot, $actor::<$state>::$c_handler as fn(&Self::Actor, serde_json::Value) -> Box<ActorStateType>);
                )*
                command_map
            }

            fn state_name() -> String {
                stringify!($state).to_string()
            }
        }
    };
}

#[macro_export]
macro_rules! declare_slots {
    ($actor:ident, [$($d_slot:expr),*]) => {
        impl<State> $actor<State>
        {
            pub fn slots() -> Vec<&'static str> {
                vec![$($d_slot),*]
            }
        }
    };
}
