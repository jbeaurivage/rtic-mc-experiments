/// A critical section that restores its previous state on drop.
///
/// # Safety
///
/// Any type implementing this trait must guarantee that no interrupt can
/// preempt whatever runs between [`enter`](Self::enter) and
/// [`restore`](Self::restore) (this includes [`exit`](Self::exit) and
/// [`drop`]).
///
/// As part of the contract for this trait, any implementer **must** also
/// implement [`Drop`], where the [`drop`] function calls
/// [`restore`](Self::restore).
pub unsafe trait DroppableCriticalSection: Sized {
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

/// A no-op token that can be used as a proof that a code section cannot be
/// preempted.
///
/// # Safety
///
/// This may only be used when we are certain that the token is created at the
/// highest (ie, most urgent) system priority, thus guaranteeing it cannot be
/// preempted.
pub(crate) struct NoopCs;

unsafe impl DroppableCriticalSection for NoopCs {
    fn enter() -> Self {
        Self
    }

    fn forget(self) {}

    fn restore(&mut self) {}
}

// Implement drop for completeness
impl Drop for NoopCs {
    fn drop(&mut self) {
        self.restore();
    }
}
