/// A critical section that restores its previous state on drop
///
/// As part of the contract for this trait, any implementer **must** also
/// implement [`Drop`], where the [`drop`] function calls
/// [`restore`](DropCriticalSection::restore).
pub trait DroppableCriticalSection: Sized {
    /// Enter the critical section by disabling the interrupts, and saving the
    /// internal state.
    fn enter() -> Self;

    fn exit(mut self) {
        self.restore();
    }

    /// Forget the cricital section without reenabling the interrupts.
    fn forget(self);

    /// Reenable the interrupts, only if they were enabled upon entering the
    /// critical section.
    fn restore(&mut self);
}
