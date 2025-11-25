pub trait CsImpl {
    fn take() -> Self;

    unsafe fn restore_inner(&mut self);
}
