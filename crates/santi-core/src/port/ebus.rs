pub trait SubscriberSetPort<S>: Send + Sync {
    fn replace_all(&self, subscribers: Vec<S>);
    fn snapshot(&self) -> Vec<S>;
}
