pub struct OnScopeExitMut<'a, T> {
    inner: &'a mut T,
    on_scope_exit: Option<Box<dyn FnOnce(&mut T) -> ()>>,
}

impl<'a, T> OnScopeExitMut<'a, T> {
    pub fn new(
        val: &'a mut T,
        on_scope_exit: Box<dyn FnOnce(&mut T) -> ()>,
    ) -> OnScopeExitMut<'a, T> {
        OnScopeExitMut {
            inner: val,
            on_scope_exit: Some(on_scope_exit),
        }
    }
}

impl<'a, T> AsMut<T> for OnScopeExitMut<'a, T> {
    fn as_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<'a, T> Drop for OnScopeExitMut<'a, T> {
    fn drop(&mut self) {
        let func = self.on_scope_exit.take().unwrap();
        func(self.inner);
    }
}
