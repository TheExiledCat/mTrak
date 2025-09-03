pub struct PromptOverlay {
    title: String,
    cancellable: bool,
}
impl PromptOverlay{
    pub fn new(title: &str, cancellable: bool)->{
        let title = title.into();
        return Self{
            title,cancellable;
        }
    }
}
