pub trait Boxed {
    fn boxed(self) -> Box<Self>
    where
        Self: Sized,
    {
        return Box::new(self);
    }
}
impl<T> Boxed for T where T: Sized {}
