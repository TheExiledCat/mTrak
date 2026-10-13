pub struct UndoStack<T>
where
    T: UndoAction,
{
    redo_stack: Stack<T>,
    undo_stack: Stack<T>,
}
impl<T> UndoStack<T>
where
    T: UndoAction,
{
    pub fn new() -> Self {
        return Self {
            redo_stack: Stack::new(),
            undo_stack: Stack::new(),
        };
    }
    pub fn track_and_do(&mut self, state: &mut T::State, mut action: T) {
        action.do_action(state);
        self.track(action);
    }
    pub fn track(&mut self, action: T) {
        self.undo_stack.push(action);
        self.redo_stack.clear();
    }
    pub fn undo(&mut self, state: &mut T::State) -> bool {
        let Some(mut action) = self.undo_stack.pop() else {
            return false;
        };
        action.undo_action(state);
        self.redo_stack.push(action);
        return true;
    }
    pub fn redo(&mut self, state: &mut T::State) -> bool {
        let Some(mut action) = self.redo_stack.pop() else {
            return false;
        };
        action.do_action(state);
        self.undo_stack.push(action);
        return true;
    }
}
impl<T> Default for UndoStack<T>
where
    T: UndoAction,
{
    fn default() -> Self {
        return Self::new();
    }
}
pub trait UndoAction {
    type State;
    fn do_action(&mut self, state: &mut Self::State);
    fn undo_action(&mut self, state: &mut Self::State);
}
pub struct Stack<T> {
    stack: Vec<T>,
}
impl<T> Stack<T> {
    pub fn new() -> Self {
        return Self { stack: Vec::new() };
    }
    pub fn push(&mut self, item: T) -> usize {
        self.stack.push(item);
        return self.stack.len();
    }
    pub fn pop(&mut self) -> Option<T> {
        return self.stack.pop();
    }
    pub fn clear(&mut self) {
        self.stack.clear();
    }
}
impl<T> Default for Stack<T> {
    fn default() -> Self {
        return Self::new();
    }
}
