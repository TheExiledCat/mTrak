use std::collections::VecDeque;

const HISTORY_LIMIT: usize = 256;

struct Entry<T> {
    id: u64,
    action: T,
}
pub struct UndoStack<T>
where
    T: UndoAction,
{
    redo_stack: Stack<Entry<T>>,
    undo_stack: Stack<Entry<T>>,
    next_id: u64,
}
impl<T> UndoStack<T>
where
    T: UndoAction,
{
    pub fn new() -> Self {
        return Self {
            redo_stack: Stack::new(HISTORY_LIMIT),
            undo_stack: Stack::new(HISTORY_LIMIT),
            next_id: 1,
        };
    }
    pub fn track_and_do(&mut self, state: &mut T::State, mut action: T) {
        action.do_action(state);
        self.track(action);
    }
    pub fn track(&mut self, action: T) {
        let id = self.next_id;
        self.next_id += 1;
        self.undo_stack.push(Entry { id, action });
        self.redo_stack.clear();
    }
    pub fn can_undo(&self) -> bool {
        return !self.undo_stack.is_empty();
    }
    pub fn can_redo(&self) -> bool {
        return !self.redo_stack.is_empty();
    }
    pub fn position(&self) -> u64 {
        return self.undo_stack.last().map_or(0, |e| e.id);
    }
    pub fn undo(&mut self, state: &mut T::State) -> Option<&T> {
        let mut entry = self.undo_stack.pop()?;
        entry.action.undo_action(state);
        self.redo_stack.push(entry);
        return self.redo_stack.last().map(|e| &e.action);
    }
    pub fn redo(&mut self, state: &mut T::State) -> Option<&T> {
        let mut entry = self.redo_stack.pop()?;
        entry.action.do_action(state);
        self.undo_stack.push(entry);
        return self.undo_stack.last().map(|e| &e.action);
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
    stack: VecDeque<T>,
    limit: usize,
}
impl<T> Stack<T> {
    pub fn new(limit: usize) -> Self {
        return Self {
            stack: VecDeque::new(),
            limit,
        };
    }
    pub fn push(&mut self, item: T) -> usize {
        self.stack.push_back(item);
        if self.stack.len() > self.limit {
            self.stack.pop_front();
        }
        return self.stack.len();
    }
    pub fn pop(&mut self) -> Option<T> {
        return self.stack.pop_back();
    }
    pub fn is_empty(&self) -> bool {
        return self.stack.is_empty();
    }
    pub fn last(&self) -> Option<&T> {
        return self.stack.back();
    }
    pub fn clear(&mut self) {
        self.stack.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Counter;
    impl UndoAction for Counter {
        type State = i32;
        fn do_action(&mut self, state: &mut i32) {
            *state += 1;
        }
        fn undo_action(&mut self, state: &mut i32) {
            *state -= 1;
        }
    }

    #[test]
    fn stack_drops_the_oldest_item_past_its_limit() {
        let mut stack = Stack::new(2);
        stack.push(1);
        stack.push(2);
        assert_eq!(stack.push(3), 2);
        assert_eq!(stack.pop(), Some(3));
        assert_eq!(stack.pop(), Some(2));
        assert_eq!(stack.pop(), None);
    }

    #[test]
    fn history_is_capped() {
        let mut history = UndoStack::new();
        let mut state = 0;
        for _ in 0..HISTORY_LIMIT + 10 {
            history.track_and_do(&mut state, Counter);
        }
        while history.undo(&mut state).is_some() {}
        assert_eq!(state, 10);
    }

    #[test]
    fn position_tells_history_branches_apart() {
        let mut history = UndoStack::new();
        let mut state = 0;
        assert_eq!(history.position(), 0);
        history.track_and_do(&mut state, Counter);
        let saved = history.position();
        history.undo(&mut state);
        assert_eq!(history.position(), 0);
        history.redo(&mut state);
        assert_eq!(history.position(), saved);
        history.undo(&mut state);
        history.track_and_do(&mut state, Counter);
        assert_ne!(history.position(), saved);
    }
}
