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
